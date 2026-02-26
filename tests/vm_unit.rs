//! Pure unit tests for VM sandboxing types and logic.
//!
//! These tests exercise functions and types that can be validated without a
//! running VM, without macOS, and without any native external dependencies.
//! They run as part of the standard `cargo test` invocation with no feature
//! flags required.
//!
//! Tested in this file:
//! - `build_vfkit_virtiofs_args()` output format and argument structure
//! - `DropEvent` serialization/deserialization round-trip
//! - `RemoteMessage` allowlist checking logic
//!
//! Because the main crate is a binary with native platform dependencies (e.g.
//! `whisper-rs`) that may not be available in all CI environments, these tests
//! define the minimal types inline rather than importing from the crate. The
//! types mirror the real implementations in `src/vm/` exactly.
//!
//! Whenever the real types change, these mirrors must be updated to match.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Inline type mirrors
//
// We duplicate the minimal type definitions so the unit tests can run on any
// platform without requiring the full crate binary to compile.
// ---------------------------------------------------------------------------

/// Mirror of src/vm/filesystem::VirtioFsShare
#[derive(Debug, Clone)]
struct VirtioFsShare {
    host_path: PathBuf,
    vm_mount_point: String,
    tag: String,
}

/// Mirror of src/vm/drop_server::DropEvent
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DropEvent {
    filename: String,
    size_bytes: u64,
    timestamp: DateTime<Utc>,
    destination_path_in_vm: PathBuf,
}

/// Mirror of src/vm/remote_channel::RemoteMessage
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemoteMessage {
    source: String,
    destination: String,
    payload: serde_json::Value,
    timestamp: DateTime<Utc>,
}

impl RemoteMessage {
    fn new(source: String, destination: String, payload: serde_json::Value) -> Self {
        Self {
            source,
            destination,
            payload,
            timestamp: Utc::now(),
        }
    }

    fn payload_size_bytes(&self) -> usize {
        self.payload.to_string().len()
    }
}

// ---------------------------------------------------------------------------
// Mirror of src/vm/filesystem::build_vfkit_virtiofs_args
//
// vfkit CLI spec:
//   --device virtio-fs,sharedDir=<host_path>,mountTag=<tag>
// ---------------------------------------------------------------------------

fn build_vfkit_virtiofs_args(share: &VirtioFsShare) -> Vec<String> {
    let device_spec = format!(
        "virtio-fs,sharedDir={},mountTag={}",
        share.host_path.display(),
        share.tag,
    );
    vec!["--device".to_string(), device_spec]
}

// ---------------------------------------------------------------------------
// Mirror of allowlist checking from src/vm/remote_channel
// ---------------------------------------------------------------------------

fn is_destination_allowed(destination: &str, allowlist: &[String]) -> bool {
    allowlist.iter().any(|allowed| allowed == destination)
}

// ---------------------------------------------------------------------------
// Tests: build_vfkit_virtiofs_args
// ---------------------------------------------------------------------------

#[test]
fn virtiofs_args_produces_two_elements() {
    let share = VirtioFsShare {
        host_path: PathBuf::from("/Users/drew/lobster-drop"),
        vm_mount_point: "/mnt/lobster-drop".to_string(),
        tag: "lobster-drop".to_string(),
    };

    let args = build_vfkit_virtiofs_args(&share);

    assert_eq!(args.len(), 2, "expected exactly two CLI arguments");
}

#[test]
fn virtiofs_args_first_element_is_device_flag() {
    let share = VirtioFsShare {
        host_path: PathBuf::from("/Users/drew/lobster-drop"),
        vm_mount_point: "/mnt/lobster-drop".to_string(),
        tag: "lobster-drop".to_string(),
    };

    let args = build_vfkit_virtiofs_args(&share);

    assert_eq!(args[0], "--device");
}

#[test]
fn virtiofs_args_device_spec_starts_with_virtio_fs() {
    let share = VirtioFsShare {
        host_path: PathBuf::from("/tmp/drop"),
        vm_mount_point: "/mnt/lobster-drop".to_string(),
        tag: "lobster-drop".to_string(),
    };

    let args = build_vfkit_virtiofs_args(&share);

    assert!(
        args[1].starts_with("virtio-fs,"),
        "device spec must start with 'virtio-fs,', got: {}",
        args[1]
    );
}

#[test]
fn virtiofs_args_device_spec_contains_shared_dir() {
    let host_path = PathBuf::from("/Users/drew/lobster-drop");
    let share = VirtioFsShare {
        host_path: host_path.clone(),
        vm_mount_point: "/mnt/lobster-drop".to_string(),
        tag: "lobster-drop".to_string(),
    };

    let args = build_vfkit_virtiofs_args(&share);

    assert!(
        args[1].contains(&format!("sharedDir={}", host_path.display())),
        "device spec must contain sharedDir=<host_path>, got: {}",
        args[1]
    );
}

#[test]
fn virtiofs_args_device_spec_contains_mount_tag() {
    let share = VirtioFsShare {
        host_path: PathBuf::from("/tmp/drop"),
        vm_mount_point: "/mnt/lobster-drop".to_string(),
        tag: "lobster-drop".to_string(),
    };

    let args = build_vfkit_virtiofs_args(&share);

    assert!(
        args[1].contains("mountTag=lobster-drop"),
        "device spec must contain mountTag=<tag>, got: {}",
        args[1]
    );
}

#[test]
fn virtiofs_args_uses_tag_not_vm_mount_point_as_mount_tag() {
    // The vfkit mountTag is the virtiofs tag string, NOT the in-VM path.
    let share = VirtioFsShare {
        host_path: PathBuf::from("/tmp/drop"),
        vm_mount_point: "/mnt/different-path".to_string(),
        tag: "my-custom-tag".to_string(),
    };

    let args = build_vfkit_virtiofs_args(&share);

    assert!(
        args[1].contains("mountTag=my-custom-tag"),
        "mountTag must use the tag field, not vm_mount_point, got: {}",
        args[1]
    );
    assert!(
        !args[1].contains("/mnt/different-path"),
        "vm_mount_point must not appear in the vfkit arg, got: {}",
        args[1]
    );
}

#[test]
fn virtiofs_args_full_format_matches_vfkit_spec() {
    // vfkit expects: --device virtio-fs,sharedDir=<path>,mountTag=<tag>
    let share = VirtioFsShare {
        host_path: PathBuf::from("/Users/drew/lobster-drop"),
        vm_mount_point: "/mnt/lobster-drop".to_string(),
        tag: "lobster-drop".to_string(),
    };

    let args = build_vfkit_virtiofs_args(&share);

    let expected_spec =
        "virtio-fs,sharedDir=/Users/drew/lobster-drop,mountTag=lobster-drop".to_string();
    assert_eq!(
        args,
        vec!["--device".to_string(), expected_spec],
        "full arg vector must match vfkit CLI spec"
    );
}

// ---------------------------------------------------------------------------
// Tests: DropEvent serialization
// ---------------------------------------------------------------------------

#[test]
fn drop_event_serializes_to_json() {
    let ts = Utc::now();
    let event = DropEvent {
        filename: "report.pdf".to_string(),
        size_bytes: 1024,
        timestamp: ts,
        destination_path_in_vm: PathBuf::from("/mnt/lobster-drop/report.pdf"),
    };

    let json = serde_json::to_string(&event).expect("serialization must succeed");

    assert!(json.contains("\"filename\""), "JSON must contain filename key");
    assert!(json.contains("report.pdf"), "JSON must contain filename value");
    assert!(json.contains("\"size_bytes\""), "JSON must contain size_bytes key");
    assert!(json.contains("1024"), "JSON must contain size_bytes value");
}

#[test]
fn drop_event_deserializes_from_json() {
    let ts = Utc::now();
    let original = DropEvent {
        filename: "notes.txt".to_string(),
        size_bytes: 42,
        timestamp: ts,
        destination_path_in_vm: PathBuf::from("/mnt/lobster-drop/notes.txt"),
    };

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: DropEvent = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.filename, original.filename);
    assert_eq!(restored.size_bytes, original.size_bytes);
    assert_eq!(restored.destination_path_in_vm, original.destination_path_in_vm);
}

#[test]
fn drop_event_round_trips_filename_unchanged() {
    let event = DropEvent {
        filename: "my document (final).docx".to_string(),
        size_bytes: 512,
        timestamp: Utc::now(),
        destination_path_in_vm: PathBuf::from("/mnt/lobster-drop/my document (final).docx"),
    };

    let json = serde_json::to_string(&event).unwrap();
    let back: DropEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(back.filename, "my document (final).docx");
}

#[test]
fn drop_event_destination_path_is_preserved() {
    let path = PathBuf::from("/mnt/lobster-drop/subdir/file.txt");
    let event = DropEvent {
        filename: "file.txt".to_string(),
        size_bytes: 0,
        timestamp: Utc::now(),
        destination_path_in_vm: path.clone(),
    };

    let json = serde_json::to_string(&event).unwrap();
    let back: DropEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(back.destination_path_in_vm, path);
}

#[test]
fn drop_event_timestamp_is_preserved() {
    // Use a fixed timestamp to avoid sub-nanosecond rounding in JSON.
    let ts = DateTime::parse_from_rfc3339("2026-01-15T10:30:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let event = DropEvent {
        filename: "file.txt".to_string(),
        size_bytes: 0,
        timestamp: ts,
        destination_path_in_vm: PathBuf::from("/mnt/lobster-drop/file.txt"),
    };

    let json = serde_json::to_string(&event).unwrap();
    let back: DropEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(back.timestamp, ts);
}

#[test]
fn drop_event_zero_size_is_valid() {
    let event = DropEvent {
        filename: "empty.txt".to_string(),
        size_bytes: 0,
        timestamp: Utc::now(),
        destination_path_in_vm: PathBuf::from("/mnt/lobster-drop/empty.txt"),
    };

    let json = serde_json::to_string(&event).unwrap();
    let back: DropEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(back.size_bytes, 0);
}

// ---------------------------------------------------------------------------
// Tests: RemoteMessage allowlist checking
// ---------------------------------------------------------------------------

#[test]
fn allowlist_permits_exact_match() {
    let allowlist = vec![
        "http://remote-lobster:8080".to_string(),
        "http://other-lobster:9000".to_string(),
    ];

    assert!(
        is_destination_allowed("http://remote-lobster:8080", &allowlist),
        "exact match must be allowed"
    );
}

#[test]
fn allowlist_blocks_non_listed_destination() {
    let allowlist = vec!["http://remote-lobster:8080".to_string()];

    assert!(
        !is_destination_allowed("http://evil-server:1234", &allowlist),
        "non-listed destination must be blocked"
    );
}

#[test]
fn allowlist_is_empty_blocks_all() {
    let allowlist: Vec<String> = vec![];

    assert!(
        !is_destination_allowed("http://any-server:8080", &allowlist),
        "empty allowlist must block everything"
    );
}

#[test]
fn allowlist_prefix_match_does_not_allow() {
    // "http://remote" should NOT match "http://remote-lobster:8080"
    let allowlist = vec!["http://remote".to_string()];

    assert!(
        !is_destination_allowed("http://remote-lobster:8080", &allowlist),
        "prefix-only match must not allow the destination"
    );
}

#[test]
fn allowlist_case_sensitive() {
    let allowlist = vec!["http://Remote-Lobster:8080".to_string()];

    assert!(
        !is_destination_allowed("http://remote-lobster:8080", &allowlist),
        "allowlist check must be case-sensitive"
    );
}

#[test]
fn allowlist_multiple_entries_all_checked() {
    let allowlist = vec![
        "http://lobster-a:8080".to_string(),
        "http://lobster-b:8080".to_string(),
        "http://lobster-c:8080".to_string(),
    ];

    assert!(is_destination_allowed("http://lobster-a:8080", &allowlist));
    assert!(is_destination_allowed("http://lobster-b:8080", &allowlist));
    assert!(is_destination_allowed("http://lobster-c:8080", &allowlist));
    assert!(!is_destination_allowed("http://lobster-d:8080", &allowlist));
}

#[test]
fn remote_message_new_sets_source_and_destination() {
    let msg = RemoteMessage::new(
        "vm-lobster".to_string(),
        "http://remote-lobster:8080".to_string(),
        serde_json::json!({"ping": true}),
    );

    assert_eq!(msg.source, "vm-lobster");
    assert_eq!(msg.destination, "http://remote-lobster:8080");
}

#[test]
fn remote_message_payload_size_bytes_is_accurate() {
    let payload = serde_json::json!({"hello": "world"});
    let expected_size = payload.to_string().len();

    let msg = RemoteMessage::new(
        "src".to_string(),
        "dest".to_string(),
        payload,
    );

    assert_eq!(msg.payload_size_bytes(), expected_size);
}

#[test]
fn remote_message_serializes_and_deserializes() {
    let msg = RemoteMessage::new(
        "vm-lobster".to_string(),
        "http://remote-lobster:8080".to_string(),
        serde_json::json!({"type": "ping", "id": 42}),
    );

    let json = serde_json::to_string(&msg).expect("serialize");
    let back: RemoteMessage = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(back.source, msg.source);
    assert_eq!(back.destination, msg.destination);
    assert_eq!(back.payload, msg.payload);
}

#[test]
fn remote_message_allowlist_check_with_real_message() {
    let allowlist = vec![
        "http://lobster-home:8080".to_string(),
        "http://lobster-office:8080".to_string(),
    ];

    let allowed_msg = RemoteMessage::new(
        "vm".to_string(),
        "http://lobster-home:8080".to_string(),
        serde_json::json!({}),
    );

    let blocked_msg = RemoteMessage::new(
        "vm".to_string(),
        "http://attacker.example.com".to_string(),
        serde_json::json!({}),
    );

    assert!(is_destination_allowed(&allowed_msg.destination, &allowlist));
    assert!(!is_destination_allowed(&blocked_msg.destination, &allowlist));
}
