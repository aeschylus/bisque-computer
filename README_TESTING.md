# Testing Guide

## Overview

The test suite has two tiers:

| Tier | File | Command | Requirements |
|------|------|---------|--------------|
| Unit tests | `tests/vm_unit.rs` | `cargo test --test vm_unit` | None |
| Integration tests | `tests/vm_integration.rs` | `cargo test --features vm-integration-tests --test vm_integration` | macOS, vfkit, Linux disk image |

---

## Unit Tests (no VM required)

Unit tests exercise pure logic — argument formatting, serialization
round-trips, and allowlist checking — without spawning any processes or
requiring any native dependencies.

```bash
cargo test --test vm_unit
```

These tests run on any platform (Linux, macOS) in any environment. They do
not require vfkit, a disk image, or any GPU/audio hardware.

---

## Integration Tests (requires macOS + vfkit)

Integration tests verify end-to-end sandbox properties: the VM really cannot
read host paths, virtio-fs shares really do propagate files, and the remote
channel relay really enforces the allowlist.

### Prerequisites

1. **macOS 13 or later** (Ventura+)
   - `Virtualization.framework` is required by vfkit.
   - Tests will not compile or run on Linux.

2. **vfkit** — the macOS Virtualization.framework CLI wrapper
   ```bash
   brew install vfkit
   ```
   Verify: `vfkit --version`

3. **A bootable Linux disk image** (raw `.img` format)
   - Must have an SSH daemon that accepts `root` login (password-less or
     key-based).
   - Must support `mount -t virtiofs lobster-drop /mnt/lobster-drop`.
   - A minimal Alpine Linux image works well.

4. **A Linux kernel and initrd** compatible with vfkit's direct Linux boot.
   - Extract from your Linux disk image.
   - Example using Alpine: `vmlinuz-lts` and `initramfs-lts`.

### Fixture files (default paths)

By default the tests look for fixture files in `tests/fixtures/`:

```
tests/fixtures/
    vmlinuz         # Linux kernel
    initrd.img      # Initial ramdisk
    test-vm.img     # Root disk image (writable copy)
```

Create the `fixtures/` directory and populate it, or set environment
variables to point to existing files.

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `BISQUE_TEST_DISK_IMAGE` | `tests/fixtures/test-vm.img` | Path to Linux disk image |
| `BISQUE_TEST_KERNEL` | `tests/fixtures/vmlinuz` | Path to Linux kernel |
| `BISQUE_TEST_INITRD` | `tests/fixtures/initrd.img` | Path to initrd |
| `BISQUE_TEST_SSH_PORT` | `2299` | Host port forwarded to guest SSH (22) |
| `BISQUE_TEST_REST_PORT` | `7799` | vfkit REST API port for the test VM |
| `BISQUE_TEST_REMOTE_URL` | `http://127.0.0.1:18080` | Allowlisted remote Lobster URL |

### Running the integration tests

```bash
# With default fixture paths
cargo test --features vm-integration-tests --test vm_integration

# With custom image paths
BISQUE_TEST_DISK_IMAGE=/path/to/alpine.img \
BISQUE_TEST_KERNEL=/path/to/vmlinuz \
BISQUE_TEST_INITRD=/path/to/initrd.img \
cargo test --features vm-integration-tests --test vm_integration

# Run a single test
cargo test --features vm-integration-tests --test vm_integration \
    test_vm_stops_cleanly -- --nocapture
```

### How the tests work

Each integration test follows the same pattern:

1. `TestVm::spawn()` launches a vfkit VM with:
   - A virtio-fs share (host `{tmp}/lobster-drop` → VM `/mnt/lobster-drop`)
   - SSH port-forwarding (host `2299` → VM `22`)
   - REST API port for lifecycle control
2. The test exercises a sandbox property via SSH commands or TCP connections.
3. `vm.stop()` sends a graceful shutdown via the vfkit REST API.
4. `TestVm::Drop` kills the VM if `stop()` was not called (e.g., on panic).

### CI configuration

Integration tests are intended to run on `macos-latest` GitHub Actions runners:

```yaml
# .github/workflows/sandbox-tests.yml
- name: Run VM integration tests
  if: runner.os == 'macOS'
  run: |
    cargo test --features vm-integration-tests --test vm_integration
  env:
    BISQUE_TEST_DISK_IMAGE: ${{ secrets.TEST_DISK_IMAGE_PATH }}
    BISQUE_TEST_KERNEL: ${{ secrets.TEST_KERNEL_PATH }}
    BISQUE_TEST_INITRD: ${{ secrets.TEST_INITRD_PATH }}
```

---

## Test descriptions

### Unit tests (`tests/vm_unit.rs`)

| Test | What it verifies |
|------|-----------------|
| `virtiofs_args_produces_two_elements` | `build_vfkit_virtiofs_args` returns exactly `["--device", "<spec>"]` |
| `virtiofs_args_first_element_is_device_flag` | First element is `--device` |
| `virtiofs_args_device_spec_starts_with_virtio_fs` | Spec starts with `virtio-fs,` |
| `virtiofs_args_device_spec_contains_shared_dir` | Spec contains `sharedDir=<host_path>` |
| `virtiofs_args_device_spec_contains_mount_tag` | Spec contains `mountTag=<tag>` |
| `virtiofs_args_uses_tag_not_vm_mount_point_as_mount_tag` | `mountTag` uses the `tag` field, not `vm_mount_point` |
| `virtiofs_args_full_format_matches_vfkit_spec` | Full arg vector matches vfkit CLI spec exactly |
| `drop_event_serializes_to_json` | `DropEvent` serializes to JSON with all fields |
| `drop_event_deserializes_from_json` | `DropEvent` deserializes from JSON correctly |
| `drop_event_round_trips_filename_unchanged` | Filename with spaces/parens survives round-trip |
| `drop_event_destination_path_is_preserved` | `destination_path_in_vm` field is preserved |
| `drop_event_timestamp_is_preserved` | Timestamp serializes/deserializes without drift |
| `drop_event_zero_size_is_valid` | Zero-byte files are valid DropEvents |
| `allowlist_permits_exact_match` | Exact URL match is allowed |
| `allowlist_blocks_non_listed_destination` | Unknown URLs are blocked |
| `allowlist_is_empty_blocks_all` | Empty allowlist blocks everything |
| `allowlist_prefix_match_does_not_allow` | Prefix match is not sufficient |
| `allowlist_case_sensitive` | Allowlist check is case-sensitive |
| `allowlist_multiple_entries_all_checked` | All entries are checked |
| `remote_message_new_sets_source_and_destination` | Constructor sets fields correctly |
| `remote_message_payload_size_bytes_is_accurate` | `payload_size_bytes()` returns JSON byte count |
| `remote_message_serializes_and_deserializes` | `RemoteMessage` round-trips through JSON |
| `remote_message_allowlist_check_with_real_message` | Full message + allowlist integration |

### Integration tests (`tests/vm_integration.rs`)

| Test | What it verifies |
|------|-----------------|
| `test_claude_cannot_read_host_passwd` | VM `/etc/passwd` does not contain host users |
| `test_drop_folder_file_appears_in_vm` | File written to host drop folder appears in VM within 2 s |
| `test_remote_channel_allowlist` | Relay rejects messages to non-allowlisted destinations |
| `test_remote_channel_relay` | Relay forwards messages to allowlisted destinations |
| `test_vm_stops_cleanly` | VM process exits within 10 s after `stop()` |
