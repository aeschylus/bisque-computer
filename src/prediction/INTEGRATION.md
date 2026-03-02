# Mosh-style Prediction Engine: Integration Guide

This guide explains how to wire `PredictionEngine` into bisque-computer's
terminal render loop. The prediction module lives in `src/prediction/` and
is self-contained — it does not depend on `alacritty_terminal` or `vello`.

---

## Architecture Overview

The prediction engine sits between key input and rendering:

```
winit KeyEvent
    │
    ▼
PredictionEngine::process_input(key)
    │
    ├── InputClass::Predictable(ch) → insert cell into overlay
    ├── InputClass::Uncertain → become_tentative() (hide subsequent predictions)
    ├── InputClass::Backspace → pop last cell, move cursor left
    ├── InputClass::Arrow{Left,Right} → move cursor prediction
    └── InputClass::Bulk → reset()
    │
    ▼
[bytes sent to WebSocket → server → tmux → shell]
    │
    (after some RTT...)
    │
    ▼
server PTY bytes arrive
    │
    ▼
PredictionEngine::process_server_output(bytes)
    │
    ├── match: remove confirmed cells from overlay
    └── mismatch: wipe all predictions, snap cursor

                    ┌──────────────┐
                    │  render loop │
                    └──────┬───────┘
                           │
                    alacritty_terminal::Term
                    (confirmed server state)
                           │
                           ▼
                    render_into_scene()
                           │
                    paint overlay cells on top
                    (PredictionEngine::overlay())
                           │
                           ▼
                    vello Scene → GPU
```

---

## Step 1: Initialize PredictionEngine

Create a `PredictionEngine` alongside `TerminalPane`. The engine needs to know
the terminal dimensions for cursor clamping.

```rust
use crate::prediction::PredictionEngine;

struct TerminalPane {
    // ... existing fields ...
    prediction: PredictionEngine,
}

impl TerminalPane {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            // ... existing init ...
            prediction: PredictionEngine::new(cols, rows),
        }
    }
}
```

---

## Step 2: Wire Key Events into the Engine

In `write_key()` (in `terminal.rs`), call `process_input` before sending bytes
to the WebSocket. Convert the winit `Key` to the engine's `KeyEvent` type.

```rust
use crate::prediction::{KeyEvent as PredKeyEvent, PredictionEngine};

pub fn write_key(&mut self, key: &Key, ctrl_held: bool) -> bool {
    let bytes = key_event_to_pty_bytes(key, ctrl_held, app_cursor);
    if bytes.is_empty() {
        return false;
    }

    // Feed the keystroke to the prediction engine BEFORE sending to server.
    // This ensures the overlay is updated synchronously with the render frame.
    let pred_key = PredKeyEvent {
        bytes: bytes.clone(),
        ctrl_held,
    };
    self.prediction.process_input(&pred_key);

    // Send the bytes to the WebSocket as before.
    self.writer.write_all(&bytes).ok();
    true
}
```

**Important:** Do not modify the bytes going to the WebSocket. The prediction
engine is purely additive — it does not alter what is sent to the server.

---

## Step 3: Feed Server Output to the Engine

In `drain_output()` (or wherever WebSocket bytes are fed into
`alacritty_terminal::Term`), also call `process_server_output`:

```rust
pub fn drain_output(&mut self) {
    while let Ok(bytes) = self.output_rx.try_recv() {
        // Feed bytes to the prediction engine for confirmation/mismatch detection.
        self.prediction.process_server_output(&bytes);

        // Feed bytes to alacritty_terminal as before.
        self.term_processor.advance(&mut self.term, &bytes);
    }

    // After processing all server output, sync the confirmed cursor position.
    let cursor = self.term.grid().cursor.point;
    self.prediction.sync_confirmed_cursor(
        cursor.column.0 as u16,
        cursor.line.0 as u16,
    );
}
```

---

## Step 4: Update RTT from WebSocket Ping/Pong

Wire RTT measurement into the engine. The WebSocket connection already has
access to ping/pong timing — update the engine when RTT changes:

```rust
// In ws_client.rs or wherever ping/pong timing is measured:
fn on_pong_received(&mut self, rtt_ms: u32) {
    self.terminal.prediction.set_rtt(rtt_ms);
}
```

The engine uses hysteresis:
- RTT >= 20ms → predictions activated
- RTT < 20ms with no pending predictions → predictions deactivated
- RTT > 80ms → predictions shown with underline (flagging mode)

---

## Step 5: Paint the Overlay in render_into_scene()

In `render_into_scene()` (in `terminal.rs`), after rendering the base
`alacritty_terminal` grid, iterate the overlay and paint on top:

```rust
pub fn render_into_scene(&self, scene: &mut Scene, /* ... */) {
    // --- Phase 1: Render confirmed terminal state (existing code) ---
    let content = self.term.renderable_content();
    for cell in content.display_iter {
        // ... existing cell rendering ...
    }

    // --- Phase 2: Paint prediction overlay on top ---
    let confirmed_epoch = self.prediction.confirmed_epoch();

    for ((col, row), overlay_cell) in self.prediction.overlay().visible_cells(confirmed_epoch) {
        let x = left_margin + col as f32 * cell_width;
        let y = top_margin + row as f32 * cell_height;

        // Use a slightly different style for predicted cells:
        // - Same color as normal text
        // - Underlined if overlay_cell.underlined (RTT > 80ms)
        render_predicted_cell(scene, x, y, overlay_cell.ch, overlay_cell.underlined);
    }

    // --- Phase 3: Paint predicted cursor ---
    let (pred_col, pred_row) = self.prediction.predicted_cursor();
    render_cursor(scene, pred_col, pred_row, /* cursor style */);
}
```

The `visible_cells(confirmed_epoch)` iterator automatically filters out cells
whose epoch has not yet been confirmed by the server. You do not need to check
epochs manually.

---

## Step 6: Handle Terminal Resize

On resize, call `set_terminal_size()` to clamp the predicted cursor and reset
all predictions (a resize invalidates everything):

```rust
pub fn resize(&mut self, cols: u16, rows: u16) {
    self.term.resize(/* ... */);
    self.prediction.set_terminal_size(cols, rows);
    // prediction.reset() is called internally by set_terminal_size()
}
```

---

## The tmux Caveat (Critical for bisque-computer)

bisque-computer always connects through tmux. This is the primary operational
difference from Mosh's direct PTY use case.

### Why tmux breaks naive prediction

1. **Modal editor problem**: When the inner pane runs vim, the user is in normal
   mode. Typing `j` moves down a line — the prediction engine will show `j` at
   the cursor, which is immediately wrong. The misprediction is visible for 1
   RTT before the server corrects it.

2. **Echo delay**: tmux adds its own processing latency even on localhost. The
   effective RTT seen by the prediction engine is slightly higher than the raw
   WebSocket RTT.

3. **tmux escape sequences**: tmux wraps inner terminal escape sequences in its
   own passthrough format. The prediction engine ignores escape sequences, which
   is correct — but it means tmux-added control bytes do not interfere.

### Mitigation: OSC 133 Shell Integration

The cleanest solution is OSC 133 shell prompt detection. When the server-side
shell emits `ESC ] 133 ; A ST` (prompt start) and `ESC ] 133 ; B ST` (prompt
end), bisque knows the inner pane is at a shell prompt — safe to predict.

**Server side (bash/zsh):**
```bash
# Add to ~/.bashrc or ~/.zshrc inside the tmux session:
PS1='\e]133;A\a'"$PS1"'\e]133;B\a'
```

**Client side (bisque-computer):**
```rust
// In drain_output(), scan for OSC 133 sequences:
fn check_shell_integration(bytes: &[u8]) -> Option<ShellPromptState> {
    // Look for ESC ] 133 ; A ST (prompt start) → ShellPromptState::AtPrompt
    // Look for ESC ] 133 ; B ST (prompt end) → ShellPromptState::Running
    // ...
}

// Gate prediction on prompt state:
if shell_state == ShellPromptState::AtPrompt {
    self.prediction.set_rtt(measured_rtt);
} else {
    // In vim, htop, etc. — disable predictions
    self.prediction.set_rtt(0); // below activation threshold
}
```

### Fallback: Alternate Screen Detection

When any application enters alternate screen mode (vim, htop, nano), the server
sends `ESC [ ? 1049 h`. The prediction engine should be suspended:

```rust
// Monitor alacritty_terminal for alternate screen mode changes:
let is_alt_screen = self.term.mode().contains(TermMode::ALT_SCREEN);
if is_alt_screen {
    self.prediction.reset();
    self.prediction.set_rtt(0); // disable
}
```

This prevents mispredictions in vim while still providing predictions at the
shell prompt (which uses the primary screen).

### Conservative Threshold for tmux

If neither OSC 133 nor alternate screen detection is implemented, use a
conservative RTT threshold:

```rust
// Only activate predictions at very high RTT (>100ms) where even brief
// mispredictions are less jarring than 200ms+ of lag.
const TMUX_CONSERVATIVE_THRESHOLD_MS: u32 = 100;

// Modify the activation check:
if measured_rtt >= TMUX_CONSERVATIVE_THRESHOLD_MS {
    prediction.set_rtt(measured_rtt);
}
```

---

## What the Server Needs to Do

**Nothing special.** The prediction engine works entirely client-side. The
server streams raw PTY bytes exactly as it does today. No protocol changes are
required for Phase 1 (single-client, byte-level confirmation).

The engine compares printable bytes in the server stream against its predicted
bytes. Control sequences from the server are ignored during comparison. This is
imprecise (it cannot distinguish "echo of typed char" from "server-generated
char that happens to match"), but works well in practice for shell prompts.

**Optional — frame sequence numbers:** For more precise confirmation, add
sequence numbers to WebSocket frames (see issue #36 for the full protocol
extension design). This allows exact per-character confirmation timing. Not
required for Phase 1.

---

## Summary of Integration Points

| Where | What |
|-------|------|
| `write_key()` | Call `prediction.process_input(&key)` before WebSocket send |
| `drain_output()` | Call `prediction.process_server_output(bytes)` and `sync_confirmed_cursor()` |
| `render_into_scene()` | Paint `overlay().visible_cells(confirmed_epoch)` on top of Term grid |
| `resize()` | Call `prediction.set_terminal_size(cols, rows)` |
| Ping/pong handler | Call `prediction.set_rtt(rtt_ms)` |
| Alternate screen detect | Call `prediction.reset()` and `set_rtt(0)` when entering alt screen |
| OSC 133 detect (optional) | Call `set_rtt(0)` when not at shell prompt |

---

## Phased Rollout Plan

**Phase 1 (current implementation):** Byte-level comparison, all predictions
visible immediately (no tentative gating by default), conservative RTT
threshold for tmux. Implement the 6 integration points above.

**Phase 2:** Add OSC 133 shell integration to gate predictions on prompt state.
Add alternate screen detection to disable predictions in vim/htop/etc.

**Phase 3:** Add frame sequence numbers to the WebSocket protocol for exact
per-character confirmation. This enables precise epoch advancement and cleaner
handling of tentative predictions after Ctrl sequences.
