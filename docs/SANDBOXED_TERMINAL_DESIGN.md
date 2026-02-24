# Sandboxed Terminal Design

## Overview

Bisque Computer's terminal panes run Claude Code (`--dangerously-skip-permissions`) as the sole interactive process inside isolated Linux VMs. The user never sees a shell prompt — Claude IS the terminal.

Each terminal session:
1. Spins up a lightweight Linux VM via Apple Virtualization.framework
2. Clones a project's `main` branch from its remote git repo
3. Launches `claude --dangerously-skip-permissions` as the primary process
4. Auto-commits work to a backup branch (high frequency) and a semantic branch (meaningful commits)
5. Tears down the VM when the pane closes

## Core Design Constraints

- **Total filesystem isolation.** The VM has its own root filesystem. It cannot access the host Mac's files. Even with `--dangerously-skip-permissions`, Claude can only affect the ephemeral VM.
- **Claude is the terminal.** No shell, no prompt. The terminal pane's PTY byte stream connects directly to Claude Code's TUI. The user interacts with Claude, not bash.
- **Projects are device-independent.** A project is a name + git URL, not a local path. Opening a project clones it fresh every time, so it works from any machine running bisque-computer.
- **Native Apple isolation.** Uses Virtualization.framework (not Docker) so no external daemon is needed. Works with Developer ID + notarization distribution.
- **Fast on Apple Silicon.** Virtualization.framework runs ARM Linux VMs natively via the hypervisor — no emulation overhead.

## Architecture

```
┌──────────────────────────────────────┐
│  bisque-computer (Rust, vello/winit) │
│                                      │
│  ┌───────────┐  ┌─────────────────┐  │
│  │ Chat UI   │  │ Terminal Pane   │  │
│  │ (local)   │  │ (vte renderer)  │  │
│  └───────────┘  └────────┬────────┘  │
│                          │            │
│              ┌───────────┴──────────┐ │
│              │ Swift FFI Bridge     │ │
│              │ (Virtualization.fw)  │ │
│              └───────────┬──────────┘ │
└──────────────────────────┼────────────┘
                           │
              VZFileHandleSerialPortAttachment
              (file descriptor pairs: read/write)
                           │
              ┌────────────▼────────────┐
              │  Lightweight Linux VM   │
              │  (Apple Silicon native) │
              │                         │
              │  /workspace/<project>   │
              │  PID 1: init script     │
              │  PID 2: claude --dsp    │
              │  PID 3: auto-commit     │
              │    daemon (background)  │
              └─────────────────────────┘
```

## I/O Data Flow

### Output (VM → screen)

```
[claude TUI output inside VM]
  → VM serial console (virtio)
  → VZFileHandleSerialPortAttachment (host file handle)
  → Swift bridge reads fd, passes bytes to Rust via callback
  → tokio::mpsc::UnboundedSender<Vec<u8>>
  → TerminalPane::drain_output() (existing code, unchanged)
  → alacritty_terminal::vte::Processor (existing, unchanged)
  → Term cell grid → vello Scene → GPU
```

### Input (keyboard → VM)

```
[winit KeyEvent]
  → TerminalPane::write_key() (existing)
  → key_event_to_pty_bytes() (existing)
  → Write to host file handle
  → VZFileHandleSerialPortAttachment forwards to VM serial console
  → claude process stdin
```

### Resize

```
[WindowEvent::Resized]
  → Calculate new cols/rows
  → Swift bridge calls VZVirtioConsoleDevice resize (or send SIGWINCH via control channel)
  → term.resize(TermSize{...}) (existing, unchanged)
```

## VM Configuration

### Guest image

A minimal Linux root filesystem containing:
- Linux kernel (ARM64, minimal config for fast boot)
- Busybox or minimal userland
- Git
- Node.js (for Claude Code)
- Claude Code CLI (`@anthropic-ai/claude-code`)

This image is bundled with the app or downloaded on first launch.

### Init script (PID 1 in the VM)

```bash
#!/bin/sh
# Clone the project
git clone "$REPO_URL" /workspace
cd /workspace
git checkout main

# Start auto-commit daemon in background
(while true; do
    git add -A
    git diff --cached --quiet || git commit -m "auto-backup $(date -Iseconds)"
    sleep 30
done) &

# Claude IS the terminal — this is the main process
exec claude --dangerously-skip-permissions
```

### Environment variables passed to VM

| Variable | Purpose |
|---|---|
| `REPO_URL` | Git clone URL for the project |
| `BRANCH` | Branch to checkout (default: `main`) |
| `BACKUP_BRANCH` | Branch name for auto-commits |
| `ANTHROPIC_API_KEY` | API key for Claude Code |

## Dual-Branch Git Strategy

### Backup branch (high frequency)
- Auto-commit every ~30 seconds via background daemon
- Captures every state change for disaster recovery
- Branch name: `bisque/backup/<project>/<session-id>`
- Pushed to remote on a longer interval (every 5 min) or on session end

### Semantic branch (meaningful commits)
- Claude Code itself commits meaningfully as it works (it already does this naturally)
- Branch name: `bisque/work/<project>/<session-id>`
- This is the branch the user would PR from
- On session end, offer to squash-merge into a clean branch

## Rust Integration Points

### TerminalPane refactoring

The existing `TerminalPane` needs to become backend-agnostic:

```rust
/// Trait for resizing the remote terminal.
pub trait PtyResize: Send {
    fn resize(&self, rows: u16, cols: u16);
}

/// Local PTY backend (existing behavior).
struct LocalPtyResize(Box<dyn portable_pty::MasterPty + Send>);

/// VM serial console backend.
struct VmPtyResize { /* calls Swift bridge to resize VM console */ }

impl TerminalPane {
    /// Existing: spawn a local shell.
    pub fn spawn(width: f64, height: f64) -> Option<Self>;

    /// New: connect to pre-wired I/O streams (used by VM backend).
    pub fn from_streams(
        rx: tokio_mpsc::UnboundedReceiver<Vec<u8>>,
        writer: Box<dyn Write + Send>,
        resizer: Box<dyn PtyResize + Send>,
        cols: usize,
        rows: usize,
    ) -> Self;
}
```

### Swift FFI bridge

A thin Swift module compiled as a static library:

```swift
// BisqueVM.swift — compiled to libbisquevm.a

import Virtualization

@_cdecl("bisque_vm_create")
public func createVM(
    kernelPath: UnsafePointer<CChar>,
    rootfsPath: UnsafePointer<CChar>,
    initScript: UnsafePointer<CChar>,
    stdinFd: UnsafeMutablePointer<Int32>,
    stdoutFd: UnsafeMutablePointer<Int32>
) -> Bool {
    // Configure VZVirtualMachineConfiguration
    // Attach serial port with VZFileHandleSerialPortAttachment
    // Return file descriptors for stdin/stdout to Rust
}

@_cdecl("bisque_vm_start")
public func startVM() -> Bool { ... }

@_cdecl("bisque_vm_stop")
public func stopVM() { ... }
```

Rust side:

```rust
// src/vm_bridge.rs
extern "C" {
    fn bisque_vm_create(
        kernel_path: *const c_char,
        rootfs_path: *const c_char,
        init_script: *const c_char,
        stdin_fd: *mut i32,
        stdout_fd: *mut i32,
    ) -> bool;
    fn bisque_vm_start() -> bool;
    fn bisque_vm_stop();
}
```

### New modules

| File | Purpose |
|---|---|
| `src/vm_bridge.rs` | Rust FFI declarations for the Swift bridge |
| `src/vm_session.rs` | VM lifecycle management, auto-commit daemon |
| `src/project.rs` | Project registry (name → git URL mapping) |
| `swift/BisqueVM.swift` | Swift bridge to Virtualization.framework |

## Distribution

### Current: Developer ID + notarization (GitHub releases)
- Virtualization.framework works freely — no restricted entitlement needed
- This is the path for immediate development

### Future: Mac App Store
- Requires `com.apple.security.virtualization` entitlement (restricted)
- Must petition Apple for approval — granted case-by-case
- Apply once the feature is stable and shipping

## Open Questions

1. **Guest image distribution.** Bundle a ~200MB Linux rootfs in the app, or download on first launch? Bundling is simpler; downloading keeps the app small.
2. **API key management.** How does `ANTHROPIC_API_KEY` get into the VM securely? Passed as env var at VM creation (never written to disk in VM), or fetched from macOS Keychain via a control channel?
3. **Session persistence.** When the user closes a terminal pane, should the VM be destroyed immediately, or kept alive in the background for a grace period?
4. **Multiple serial ports.** Use one serial port for the interactive terminal and a second for structured control messages (resize, shutdown, status)?
5. **Rosetta in VM.** Virtualization.framework supports Rosetta for running x86_64 binaries in ARM VMs. Needed for any x86-only tools in the guest?
