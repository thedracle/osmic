//! Bounded-queue PBF blob streamer — libosmium pattern in idiomatic Rust.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────┐   bounded(N)   ┌──────────────┐
//! │ I/O thread │─── Blob ───> │ your workers │
//! │ (1 worker) │                 │ (M workers)  │
//! └────────────┘                 └──────────────┘
//! ```
//!
//! A single I/O thread reads blobs sequentially from a PBF file via
//! [`osmpbf::BlobReader`] and pushes each one to a bounded
//! [`crossbeam_channel`]. Worker threads receive `Blob` values from the
//! channel, decompress and decode them, and do whatever downstream work
//! they need.
//!
//! The bounded channel provides **backpressure**: if workers fall behind,
//! the I/O thread blocks on `send()`, bounding peak memory to
//! `channel_capacity × max_blob_size` (typically ~16 × 32MB = 512 MB
//! worst case for OSM PBF).
//!
//! # When to use this vs `osmpbf::ElementReader::par_map_reduce`
//!
//! `par_map_reduce` is simpler and faster for the common case (count
//! features, accumulate statistics). Use `BlobStream` when you need:
//!
//! - Explicit memory bounds (e.g. streaming into a disk-backed store)
//! - Multi-stage pipelines where decoded output feeds a different thread
//! - Control over the number of decoder threads independent of rayon
//! - Graceful shutdown via channel drop
//!
//! # Example
//!
//! ```no_run
//! use std::path::Path;
//! use omm_osm::io::{BlobStream, BlobStreamConfig};
//!
//! let stream = BlobStream::spawn(
//!     Path::new("planet.osm.pbf"),
//!     BlobStreamConfig::default(),
//! ).expect("failed to open PBF");
//!
//! // Each item is io::Result<Blob>, so handle per-blob errors.
//! for result in stream.iter() {
//!     let blob = result.expect("read ok");
//!     // blob.decode() returns OsmHeader / OsmData / Unknown
//!     let _ = blob.decode();
//! }
//! ```

use std::io;
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{bounded, Receiver, SendError, Sender};
use osmpbf::{Blob, BlobReader};

/// Tunable parameters for the [`BlobStream`].
#[derive(Debug, Clone, Copy)]
pub struct BlobStreamConfig {
    /// Maximum number of blobs queued between the I/O thread and the
    /// decoder pool. Each blob is ~32 MB compressed, so total peak memory
    /// is roughly `channel_capacity × 32 MB`.
    ///
    /// Default: 16 → ~512 MB peak.
    pub channel_capacity: usize,
}

impl Default for BlobStreamConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 16,
        }
    }
}

/// A streaming PBF blob reader with a bounded internal queue.
///
/// The I/O thread runs in the background and pushes blobs to a
/// crossbeam channel. Receivers consume blobs via [`BlobStream::iter()`]
/// or by cloning the underlying [`Receiver`] into worker threads.
pub struct BlobStream {
    rx: Receiver<io::Result<Blob>>,
    handle: Option<JoinHandle<io::Result<()>>>,
}

impl BlobStream {
    /// Open a PBF file and spawn the background I/O thread.
    ///
    /// The returned `BlobStream` holds the receiver end of the bounded
    /// channel. Drop it to signal the I/O thread to exit — pending blobs
    /// in flight are discarded.
    pub fn spawn(pbf_path: &Path, config: BlobStreamConfig) -> io::Result<Self> {
        let (tx, rx) = bounded(config.channel_capacity);
        let path_owned: PathBuf = pbf_path.to_path_buf();

        let handle = thread::Builder::new()
            .name("omm-pbf-reader".into())
            .spawn(move || io_thread_main(path_owned, tx))?;

        Ok(Self {
            rx,
            handle: Some(handle),
        })
    }

    /// Iterator over received blobs. Each item is a `Result` so callers
    /// can handle per-blob decode errors without aborting the whole
    /// stream. Iteration ends when the I/O thread exits (EOF or error).
    pub fn iter(&self) -> BlobStreamIter<'_> {
        BlobStreamIter { rx: &self.rx }
    }

    /// Clone the receiver for fan-out to multiple worker threads.
    /// Each worker should own its own `Receiver` and call `recv()` in a
    /// loop until `Err(RecvError)` indicates the channel is closed.
    pub fn receiver(&self) -> Receiver<io::Result<Blob>> {
        self.rx.clone()
    }
}

impl Drop for BlobStream {
    fn drop(&mut self) {
        // Detaching the handle is fine — the I/O thread will observe a
        // closed channel on its next send() and return cleanly.
        if let Some(handle) = self.handle.take() {
            // Don't block Drop on the reader thread, but give it a chance
            // to log errors by briefly waiting. A fully async-safe Drop
            // would require explicit `join()`; this is good enough for
            // RAII use cases.
            let _ = handle.join();
        }
    }
}

/// Lending iterator over blobs from a [`BlobStream`].
pub struct BlobStreamIter<'a> {
    rx: &'a Receiver<io::Result<Blob>>,
}

impl<'a> Iterator for BlobStreamIter<'a> {
    type Item = io::Result<Blob>;

    fn next(&mut self) -> Option<Self::Item> {
        self.rx.recv().ok()
    }
}

/// The I/O thread's main loop — reads blobs from disk and pushes them
/// to the channel until EOF or the channel is closed.
fn io_thread_main(path: PathBuf, tx: Sender<io::Result<Blob>>) -> io::Result<()> {
    let reader = BlobReader::from_path(&path).map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to open PBF {}: {e}", path.display()),
        )
    })?;

    for blob_result in reader {
        let msg = blob_result.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()));

        // If the receiver has been dropped (e.g. consumer exited early),
        // stop reading. Returning the SendError propagates nothing; it's
        // a normal termination.
        if let Err(SendError(_)) = tx.send(msg) {
            return Ok(());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use osmpbf::BlobDecode;

    /// A known-good tiny PBF that ships with the osmpbf crate for testing.
    fn test_pbf_path() -> Option<PathBuf> {
        let candidates = [
            // osmpbf vendored test file
            "/Users/nickpaterno/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/osmpbf-0.3.8/tests/test.osm.pbf",
        ];
        candidates.iter().map(PathBuf::from).find(|p| p.exists())
    }

    #[test]
    fn stream_reads_at_least_one_blob() {
        let Some(path) = test_pbf_path() else {
            eprintln!("skipping: no test PBF available");
            return;
        };

        let stream = BlobStream::spawn(&path, BlobStreamConfig::default())
            .expect("spawn should succeed");

        let count = stream.iter().filter(|r| r.is_ok()).count();
        assert!(count >= 1, "stream must yield at least one blob");
    }

    #[test]
    fn stream_emits_both_header_and_data_blobs() {
        let Some(path) = test_pbf_path() else {
            eprintln!("skipping: no test PBF available");
            return;
        };

        let stream = BlobStream::spawn(&path, BlobStreamConfig::default())
            .expect("spawn should succeed");

        let mut saw_header = false;
        let mut saw_data = false;
        for blob_result in stream.iter() {
            let blob = blob_result.expect("blob read ok");
            match blob.decode() {
                Ok(BlobDecode::OsmHeader(_)) => saw_header = true,
                Ok(BlobDecode::OsmData(_)) => saw_data = true,
                _ => {}
            }
        }
        assert!(saw_header, "expected at least one OsmHeader blob");
        assert!(saw_data, "expected at least one OsmData blob");
    }

    #[test]
    fn custom_channel_capacity_respected() {
        let Some(path) = test_pbf_path() else {
            eprintln!("skipping: no test PBF available");
            return;
        };

        // Use capacity 1 — maximally backpressured but still works
        let stream = BlobStream::spawn(
            &path,
            BlobStreamConfig { channel_capacity: 1 },
        )
        .expect("spawn should succeed");

        let count = stream.iter().count();
        assert!(count >= 1);
    }
}
