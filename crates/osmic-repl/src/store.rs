use std::path::Path;

use redb::{Database, ReadableTableMetadata, TableDefinition};

use osmic_core::bbox::BBox;
use osmic_core::error::{OsmicError, OsmicResult};

/// redb table: osm_id (i64) -> bbox bytes (4 x f64 = 32 bytes).
/// We store the bbox so we can mark dirty tiles on delete without
/// needing the full feature.
const BBOXES: TableDefinition<i64, &[u8]> = TableDefinition::new("bboxes");

/// Persistent bbox store backed by redb.
///
/// For incremental tile updates, we only need to know WHICH tiles are
/// affected by a change. We store the bounding box of each feature by
/// OSM ID. The full feature data (geometry, tags) lives in the PMTiles
/// output and is regenerated from the PBF + changes.
pub struct FeatureStore {
    db: Database,
}

impl FeatureStore {
    /// Open or create a feature store at the given path.
    pub fn open(path: &Path) -> OsmicResult<Self> {
        let db = Database::create(path)
            .map_err(|e| OsmicError::Other(format!("Failed to open feature store: {e}")))?;

        // Ensure table exists
        let txn = db
            .begin_write()
            .map_err(|e| OsmicError::Other(format!("Failed to begin write: {e}")))?;
        {
            let _table = txn
                .open_table(BBOXES)
                .map_err(|e| OsmicError::Other(format!("Failed to open table: {e}")))?;
        }
        txn.commit()
            .map_err(|e| OsmicError::Other(format!("Failed to commit: {e}")))?;

        Ok(Self { db })
    }

    /// Store a feature's bounding box. Returns the old bbox if it existed.
    pub fn upsert(&self, id: i64, bbox: &BBox) -> OsmicResult<Option<BBox>> {
        let old = self.get_bbox(id)?;

        let bytes = bbox_to_bytes(bbox);
        let txn = self
            .db
            .begin_write()
            .map_err(|e| OsmicError::Other(format!("Write txn failed: {e}")))?;
        {
            let mut table = txn
                .open_table(BBOXES)
                .map_err(|e| OsmicError::Other(format!("Open table failed: {e}")))?;
            table
                .insert(id, bytes.as_slice())
                .map_err(|e| OsmicError::Other(format!("Insert failed: {e}")))?;
        }
        txn.commit()
            .map_err(|e| OsmicError::Other(format!("Commit failed: {e}")))?;

        Ok(old)
    }

    /// Delete a feature's bbox by ID. Returns the old bbox if it existed.
    pub fn delete(&self, id: i64) -> OsmicResult<Option<BBox>> {
        let old = self.get_bbox(id)?;

        let txn = self
            .db
            .begin_write()
            .map_err(|e| OsmicError::Other(format!("Write txn failed: {e}")))?;
        {
            let mut table = txn
                .open_table(BBOXES)
                .map_err(|e| OsmicError::Other(format!("Open table failed: {e}")))?;
            let _ = table.remove(id);
        }
        txn.commit()
            .map_err(|e| OsmicError::Other(format!("Commit failed: {e}")))?;

        Ok(old)
    }

    /// Get a stored bbox by ID.
    pub fn get_bbox(&self, id: i64) -> OsmicResult<Option<BBox>> {
        let txn = self
            .db
            .begin_read()
            .map_err(|e| OsmicError::Other(format!("Read txn failed: {e}")))?;
        let table = txn
            .open_table(BBOXES)
            .map_err(|e| OsmicError::Other(format!("Open table failed: {e}")))?;

        match table.get(id) {
            Ok(Some(guard)) => {
                let bytes = guard.value();
                Ok(Some(bbox_from_bytes(bytes)))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(OsmicError::Other(format!("Get failed: {e}"))),
        }
    }

    /// Count of features in the store.
    pub fn len(&self) -> OsmicResult<u64> {
        let txn = self
            .db
            .begin_read()
            .map_err(|e| OsmicError::Other(format!("Read txn failed: {e}")))?;
        let table = txn
            .open_table(BBOXES)
            .map_err(|e| OsmicError::Other(format!("Open table failed: {e}")))?;
        Ok(table.len().unwrap_or(0))
    }

    pub fn is_empty(&self) -> OsmicResult<bool> {
        self.len().map(|n| n == 0)
    }
}

fn bbox_to_bytes(bbox: &BBox) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&bbox.min_lon.to_le_bytes());
    buf[8..16].copy_from_slice(&bbox.min_lat.to_le_bytes());
    buf[16..24].copy_from_slice(&bbox.max_lon.to_le_bytes());
    buf[24..32].copy_from_slice(&bbox.max_lat.to_le_bytes());
    buf
}

fn bbox_from_bytes(bytes: &[u8]) -> BBox {
    BBox {
        min_lon: f64::from_le_bytes(bytes[0..8].try_into().unwrap()),
        min_lat: f64::from_le_bytes(bytes[8..16].try_into().unwrap()),
        max_lon: f64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        max_lat: f64::from_le_bytes(bytes[24..32].try_into().unwrap()),
    }
}
