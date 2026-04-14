use std::path::Path;

use omm_core::error::{OmmError, OmmResult};
use serde::{Deserialize, Serialize};

/// Tracks replication state: current sequence number and base URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationState {
    pub sequence_number: u64,
    pub timestamp: String,
    pub base_url: String,
}

impl ReplicationState {
    /// Load replication state from a state directory.
    pub fn load(state_dir: &Path) -> OmmResult<Self> {
        let path = state_dir.join("state.json");
        let data = std::fs::read_to_string(&path)
            .map_err(|e| OmmError::Other(format!("Failed to read state: {e}")))?;
        serde_json::from_str(&data)
            .map_err(|e| OmmError::Other(format!("Failed to parse state: {e}")))
    }

    /// Save replication state to a state directory.
    pub fn save(&self, state_dir: &Path) -> OmmResult<()> {
        std::fs::create_dir_all(state_dir)
            .map_err(|e| OmmError::Other(format!("Failed to create state dir: {e}")))?;
        let path = state_dir.join("state.json");
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| OmmError::Other(format!("Failed to serialize state: {e}")))?;
        std::fs::write(&path, data)
            .map_err(|e| OmmError::Other(format!("Failed to write state: {e}")))
    }

    /// Compute the URL for the next .osc.gz replication file.
    ///
    /// OSM replication files use a 9-digit sequence number split into
    /// 3-digit directory segments: 000/001/234.osc.gz
    pub fn next_osc_url(&self) -> String {
        let seq = self.sequence_number + 1;
        let a = seq / 1_000_000;
        let b = (seq / 1_000) % 1_000;
        let c = seq % 1_000;
        format!(
            "{}/{:03}/{:03}/{:03}.osc.gz",
            self.base_url.trim_end_matches('/'),
            a, b, c
        )
    }

    /// Create initial state for a given base URL and sequence number.
    pub fn init(base_url: &str, sequence_number: u64) -> Self {
        Self {
            sequence_number,
            timestamp: String::new(),
            base_url: base_url.to_string(),
        }
    }
}
