//! Drop folder server: file ingestion for the Lobster VM.
//!
//! The drop folder is the only path shared between the host and the VM
//! (via virtio-fs). Files written to the host-side drop folder appear
//! instantly inside the VM at `/mnt/lobster-drop/`.
//!
//! A [`DropEvent`] is emitted whenever a new file arrives in the drop
//! folder, either via HTTP upload or via filesystem watch (FSEvents on
//! macOS).

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A file that has been delivered into the drop folder.
///
/// Emitted on the broadcast channel returned by the drop server whenever a
/// new file arrives, regardless of which ingestion path was used (HTTP upload
/// or Finder drag).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropEvent {
    /// Bare filename (no directory component).
    pub filename: String,
    /// File size in bytes at the time of detection.
    pub size_bytes: u64,
    /// UTC timestamp of detection.
    pub timestamp: DateTime<Utc>,
    /// Absolute path inside the VM where the file will appear.
    pub destination_path_in_vm: PathBuf,
}

impl DropEvent {
    /// Construct a new `DropEvent` for a file that has just appeared.
    pub fn new(filename: String, size_bytes: u64, vm_drop_folder: &PathBuf) -> Self {
        Self {
            destination_path_in_vm: vm_drop_folder.join(&filename),
            filename,
            size_bytes,
            timestamp: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_event_serializes_and_deserializes() {
        let ts = DateTime::parse_from_rfc3339("2026-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let event = DropEvent {
            filename: "test.txt".to_owned(),
            size_bytes: 42,
            timestamp: ts,
            destination_path_in_vm: PathBuf::from("/mnt/lobster-drop/test.txt"),
        };

        let json = serde_json::to_string(&event).expect("serialise");
        let back: DropEvent = serde_json::from_str(&json).expect("deserialise");

        assert_eq!(back.filename, "test.txt");
        assert_eq!(back.size_bytes, 42);
        assert_eq!(back.timestamp, ts);
        assert_eq!(
            back.destination_path_in_vm,
            PathBuf::from("/mnt/lobster-drop/test.txt")
        );
    }
}
