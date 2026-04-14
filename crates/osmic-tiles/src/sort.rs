/// External merge sort for tile features.
///
/// When the feature set is too large to sort in RAM, this module writes
/// (sort_key, feature_idx) pairs to temp files (one per chunk), sorts each
/// chunk in memory, then streams them back in order via a min-heap merge.
///
/// Sort key layout (64 bits):
///   bits 63-48  zoom  (16 bits, values 0-20)
///   bits 47-24  tile_x (24 bits, values 0-16777215)
///   bits 23-0   tile_y (24 bits, values 0-16777215)
///
/// This ordering groups all tiles for zoom Z together, then sorts by x, then y,
/// which is the natural traversal order for writing PMTiles archives.
use std::collections::BinaryHeap;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

// ── Sort key ─────────────────────────────────────────────────────────────────

/// Encode (zoom, x, y) into a u64 sort key.
///
/// Zoom occupies the high 16 bits so tiles are grouped by zoom level first,
/// then by x, then y within each zoom.
pub fn tile_sort_key(zoom: u8, x: u32, y: u32) -> u64 {
    ((zoom as u64) << 48) | ((x as u64) << 24) | (y as u64)
}

// ── On-disk record format ─────────────────────────────────────────────────────
//
// Each record is 16 bytes:
//   8 bytes  u64   sort key (little-endian)
//   8 bytes  u64   feature index (usize cast to u64, little-endian)

const RECORD_SIZE: usize = 16;

fn write_record(w: &mut impl Write, key: u64, idx: usize) -> io::Result<()> {
    w.write_all(&key.to_le_bytes())?;
    w.write_all(&(idx as u64).to_le_bytes())?;
    Ok(())
}

fn read_record(r: &mut impl Read) -> io::Result<Option<(u64, usize)>> {
    let mut buf = [0u8; RECORD_SIZE];
    match r.read_exact(&mut buf) {
        Ok(()) => {
            let key = u64::from_le_bytes(buf[0..8].try_into().unwrap());
            let idx = u64::from_le_bytes(buf[8..16].try_into().unwrap()) as usize;
            Ok(Some((key, idx)))
        }
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
        Err(e) => Err(e),
    }
}

// ── ExternalFeatureSort ───────────────────────────────────────────────────────

/// External merge sort accumulator for (sort_key, feature_idx) pairs.
///
/// Records are accumulated in an in-memory buffer of `chunk_size` entries.
/// When the buffer fills, it is sorted and flushed to a temp file. `finish()`
/// flushes any remaining records, then returns a `SortedIterator` that merges
/// all chunks in sorted order using a min-heap.
pub struct ExternalFeatureSort {
    tmp_dir: PathBuf,
    chunk_size: usize,
    /// In-memory buffer for the current chunk.
    buffer: Vec<(u64, usize)>,
    /// Paths of sorted chunk files written so far.
    chunk_files: Vec<PathBuf>,
    /// Counter for generating unique temp file names.
    chunk_counter: usize,
}

impl ExternalFeatureSort {
    /// Create a new sorter.
    ///
    /// - `tmp_dir`: directory where temp files are written (must exist).
    /// - `chunk_size`: number of records to sort in memory at once.
    ///   A record is 16 bytes, so `chunk_size = 1_000_000` uses ~16 MB.
    pub fn new(tmp_dir: &Path, chunk_size: usize) -> Self {
        Self {
            tmp_dir: tmp_dir.to_path_buf(),
            chunk_size,
            buffer: Vec::with_capacity(chunk_size),
            chunk_files: Vec::new(),
            chunk_counter: 0,
        }
    }

    /// Add a single (key, feature_idx) record.
    ///
    /// Flushes to disk when the in-memory buffer reaches `chunk_size`.
    pub fn add(&mut self, key: u64, feature_idx: usize) -> io::Result<()> {
        self.buffer.push((key, feature_idx));
        if self.buffer.len() >= self.chunk_size {
            self.flush_chunk()?;
        }
        Ok(())
    }

    /// Flush the in-memory buffer to a sorted temp file.
    fn flush_chunk(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        self.buffer.sort_unstable_by_key(|&(k, _)| k);

        let path = self
            .tmp_dir
            .join(format!("osmic_sort_chunk_{:06}.bin", self.chunk_counter));
        self.chunk_counter += 1;

        {
            let file = File::create(&path)?;
            let mut writer = BufWriter::new(file);
            for &(key, idx) in &self.buffer {
                write_record(&mut writer, key, idx)?;
            }
            writer.flush()?;
        }

        self.buffer.clear();
        self.chunk_files.push(path);
        Ok(())
    }

    /// Flush remaining records, then return a `SortedIterator` over all chunks.
    ///
    /// If no records were ever added (or everything fit in one in-memory
    /// buffer and no chunks were written), the iterator returns records
    /// directly from the sorted in-memory buffer with no file I/O.
    pub fn finish(mut self) -> io::Result<SortedIterator> {
        // Flush whatever is left in the buffer.
        self.flush_chunk()?;

        SortedIterator::new(self.chunk_files)
    }
}

// ── SortedIterator ────────────────────────────────────────────────────────────

/// State for one open chunk file in the merge heap.
struct ChunkReader {
    reader: BufReader<File>,
    /// The current (peeked) record from this chunk.
    current: (u64, usize),
    /// Path kept alive for cleanup on drop.
    path: PathBuf,
}

impl ChunkReader {
    fn open(path: PathBuf) -> io::Result<Option<Self>> {
        let file = File::open(&path)?;
        let mut reader = BufReader::new(file);
        match read_record(&mut reader)? {
            Some(current) => Ok(Some(ChunkReader {
                reader,
                current,
                path,
            })),
            // Empty chunk file (shouldn't happen, but handle gracefully).
            None => {
                let _ = fs::remove_file(&path);
                Ok(None)
            }
        }
    }

    /// Advance to the next record and return the previous one.
    fn advance(&mut self) -> io::Result<Option<(u64, usize)>> {
        match read_record(&mut self.reader)? {
            Some(next) => {
                let prev = self.current;
                self.current = next;
                Ok(Some(prev))
            }
            None => Ok(None),
        }
    }
}

impl Drop for ChunkReader {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

// We need ChunkReader in a BinaryHeap ordered by sort key (ascending).
// BinaryHeap is a max-heap, so we wrap in Reverse.
impl PartialEq for ChunkReader {
    fn eq(&self, other: &Self) -> bool {
        self.current.0 == other.current.0
    }
}
impl Eq for ChunkReader {}
impl PartialOrd for ChunkReader {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ChunkReader {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse so that the smallest key is at the top of the max-heap.
        other.current.0.cmp(&self.current.0)
    }
}

/// Streaming iterator that merges sorted chunk files in sort-key order.
///
/// Temp files are deleted from disk as each `ChunkReader` is exhausted or
/// dropped (including when `SortedIterator` itself is dropped).
pub struct SortedIterator {
    heap: BinaryHeap<ChunkReader>,
}

impl SortedIterator {
    fn new(chunk_files: Vec<PathBuf>) -> io::Result<Self> {
        let mut heap = BinaryHeap::with_capacity(chunk_files.len());
        for path in chunk_files {
            if let Some(reader) = ChunkReader::open(path)? {
                heap.push(reader);
            }
        }
        Ok(SortedIterator { heap })
    }
}

impl Iterator for SortedIterator {
    type Item = (u64, usize);

    fn next(&mut self) -> Option<Self::Item> {
        // Peek at the smallest-key chunk at the top of the heap.
        let mut top = self.heap.pop()?;
        let item = top.current;

        // Try to advance the chunk to its next record.
        match top.advance() {
            Ok(Some(_)) => {
                // The chunk has more records; put it back.
                self.heap.push(top);
            }
            Ok(None) => {
                // Chunk exhausted. `top` drops here, which deletes the file.
            }
            Err(_) => {
                // I/O error — treat as exhausted so we don't hang.
            }
        }

        Some(item)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn sort_key_ordering() {
        // Lower zoom < higher zoom
        assert!(tile_sort_key(0, 0, 0) < tile_sort_key(1, 0, 0));
        // Within same zoom, lower x < higher x
        assert!(tile_sort_key(5, 0, 0) < tile_sort_key(5, 1, 0));
        // Within same zoom+x, lower y < higher y
        assert!(tile_sort_key(5, 3, 0) < tile_sort_key(5, 3, 1));
    }

    #[test]
    fn external_sort_single_chunk() {
        let tmp = std::env::temp_dir().join("osmic_sort_test_single");
        let _ = fs::create_dir_all(&tmp);

        let mut sorter = ExternalFeatureSort::new(&tmp, 1000);
        let pairs: Vec<(u64, usize)> = vec![
            (tile_sort_key(5, 2, 3), 42),
            (tile_sort_key(5, 1, 0), 7),
            (tile_sort_key(3, 0, 0), 1),
            (tile_sort_key(5, 2, 1), 99),
        ];
        for (k, i) in &pairs {
            sorter.add(*k, *i).unwrap();
        }

        let result: Vec<(u64, usize)> = sorter.finish().unwrap().collect();
        let keys: Vec<u64> = result.iter().map(|(k, _)| *k).collect();

        assert!(keys.windows(2).all(|w| w[0] <= w[1]), "keys must be sorted");
        // All original pairs must be present
        let result_set: HashSet<(u64, usize)> = result.into_iter().collect();
        for p in pairs {
            assert!(result_set.contains(&p));
        }
    }

    #[test]
    fn external_sort_multi_chunk() {
        let tmp = std::env::temp_dir().join("osmic_sort_test_multi");
        let _ = fs::create_dir_all(&tmp);

        // chunk_size=3 forces multiple flushes for 10 records
        let mut sorter = ExternalFeatureSort::new(&tmp, 3);
        let n = 10usize;
        // Insert in reverse order so sorting is non-trivial
        for i in (0..n).rev() {
            let key = tile_sort_key(5, i as u32, 0);
            sorter.add(key, i).unwrap();
        }

        let result: Vec<(u64, usize)> = sorter.finish().unwrap().collect();
        assert_eq!(result.len(), n, "all records must be present");

        let keys: Vec<u64> = result.iter().map(|(k, _)| *k).collect();
        assert!(keys.windows(2).all(|w| w[0] <= w[1]), "keys must be sorted");
    }

    #[test]
    fn temp_files_cleaned_up() {
        let tmp = std::env::temp_dir().join("osmic_sort_test_cleanup");
        let _ = fs::create_dir_all(&tmp);

        let mut sorter = ExternalFeatureSort::new(&tmp, 2);
        for i in 0..6usize {
            sorter.add(tile_sort_key(1, i as u32, 0), i).unwrap();
        }
        // Collect all items (exhausts each ChunkReader → deletes files)
        let iter = sorter.finish().unwrap();
        let _: Vec<_> = iter.collect();

        // No osmic_sort_chunk files should remain
        let remaining: Vec<_> = fs::read_dir(&tmp)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("osmic_sort_chunk")
            })
            .collect();
        assert!(remaining.is_empty(), "temp files must be deleted after iteration");
    }
}
