//! Mosh-style local keystroke prediction for bisque-computer.
//!
//! This module implements speculative local echo — the same algorithm used by
//! Mosh — to eliminate perceived latency on high-RTT connections. When a user
//! types a character, the prediction engine immediately renders it locally
//! without waiting for the server to echo it back. If the server disagrees,
//! the prediction is silently discarded.
//!
//! ## Architecture
//!
//! ```text
//!  KeyEvent ──► process_input()
//!                     │
//!                     ▼
//!              classify (input.rs)
//!                     │
//!            ┌────────┼────────────┐
//!            │        │            │
//!         Predictable Uncertain  Bulk/Backspace/Arrow
//!            │        │
//!         insert     become_tentative()
//!         into        │
//!         overlay     (new predictions hidden until
//!         (overlay.rs) server confirms)
//!
//!  Server bytes ──► process_server_output()
//!                         │
//!                    compare to overlay
//!                    ┌────┴─────────┐
//!                  Match         Mismatch
//!                    │               │
//!             advance_confirmed()  kill_epoch() / reset()
//! ```
//!
//! ## RTT gating
//!
//! The engine is inactive below 20ms RTT (predictions never shown). Between
//! 20ms and 80ms, predictions are shown without underline. Above 80ms,
//! predictions are underlined to indicate uncertainty.
//!
//! ## Integration
//!
//! See `INTEGRATION.md` in this module directory for detailed wiring instructions.

pub mod epoch;
pub mod input;
pub mod overlay;

#[cfg(test)]
mod tests;

pub use epoch::EpochTracker;
pub use input::{InputClass, KeyEvent, classify};
pub use overlay::{FrameOverlay, OverlayCell, PredictedCursor};

/// RTT threshold below which predictions are suppressed entirely.
const RTT_THRESHOLD_LOW_MS: u32 = 20;

/// RTT threshold above which predictions are displayed with underline.
const RTT_THRESHOLD_HIGH_MS: u32 = 80;

/// Hysteresis threshold: once predictions were active, keep them active
/// until RTT drops below this value (prevents flicker near the boundary).
const RTT_HYSTERESIS_MS: u32 = 20;

/// Byte value for DEL / backspace.
const BACKSPACE_BYTE: u8 = 0x7f;

/// The prediction engine for Mosh-style local keystroke prediction.
///
/// This struct owns all mutable prediction state. It exposes a small,
/// pure-ish API:
/// - `process_input()` — feed a keystroke, get back an optional overlay action
/// - `process_server_output()` — feed confirmed server bytes to advance epochs
/// - `overlay()` — the current overlay to paint on top of confirmed state
/// - `set_rtt()` — update RTT estimate for activation gating
/// - `reset()` — wipe all state (on paste, resize, etc.)
///
/// The engine does NOT mutate `alacritty_terminal::Term`. It maintains its
/// own separate cursor position and cell predictions.
pub struct PredictionEngine {
    /// The epoch tracker — core of the confirmation model.
    epochs: EpochTracker,

    /// The overlay grid — sparse map of predicted cells.
    overlay: FrameOverlay,

    /// Current predicted cursor column (0-indexed).
    cursor_col: u16,

    /// Current predicted cursor row (0-indexed).
    cursor_row: u16,

    /// The confirmed cursor column — updated when server output is processed.
    confirmed_cursor_col: u16,

    /// The confirmed cursor row — updated when server output is processed.
    confirmed_cursor_row: u16,

    /// Terminal width (columns). Used for right-edge clamping.
    terminal_cols: u16,

    /// Terminal height (rows). Used for bottom-edge clamping.
    terminal_rows: u16,

    /// Smoothed RTT estimate in milliseconds.
    rtt_ms: u32,

    /// Whether predictions are currently active (RTT-gated).
    ///
    /// Uses hysteresis: activates when RTT >= RTT_THRESHOLD_LOW_MS,
    /// deactivates only when RTT < RTT_HYSTERESIS_MS AND no predictions pending.
    predictions_active: bool,

    /// Buffer of predicted bytes in order, for comparing against server output.
    ///
    /// Each entry is the byte we predicted at that position. When the server
    /// sends output, we compare it character by character against this buffer.
    predicted_bytes: Vec<u8>,
}

/// The action taken by the prediction engine after processing input.
///
/// Returned by `process_input()` for inspection in tests and callers that
/// want to know what the engine did without inspecting the overlay directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PredictionAction {
    /// A character was predicted and added to the overlay.
    CharInserted { col: u16, row: u16, ch: char },

    /// The cursor prediction was moved left (backspace or left arrow).
    CursorMovedLeft,

    /// The cursor prediction was moved right (right arrow).
    CursorMovedRight,

    /// The engine became tentative — predictions hidden until server confirms.
    BecameTentative,

    /// The engine was fully reset (bulk paste or overflow).
    Reset,

    /// Predictions are inactive at current RTT — no action taken.
    Inactive,
}

impl PredictionEngine {
    /// Create a new prediction engine.
    ///
    /// The engine starts inactive (RTT is 0ms). Call `set_rtt()` to enable it.
    ///
    /// `terminal_cols` and `terminal_rows` define the grid dimensions for
    /// cursor clamping. Call `set_terminal_size()` on resize.
    pub fn new(terminal_cols: u16, terminal_rows: u16) -> Self {
        Self {
            epochs: EpochTracker::new(),
            overlay: FrameOverlay::new(),
            cursor_col: 0,
            cursor_row: 0,
            confirmed_cursor_col: 0,
            confirmed_cursor_row: 0,
            terminal_cols,
            terminal_rows,
            rtt_ms: 0,
            predictions_active: false,
            predicted_bytes: Vec::new(),
        }
    }

    /// Update the RTT estimate and adjust prediction activation accordingly.
    ///
    /// - RTT < 20ms with no pending predictions → deactivate
    /// - RTT >= 20ms → activate
    pub fn set_rtt(&mut self, rtt_ms: u32) {
        self.rtt_ms = rtt_ms;

        if rtt_ms >= RTT_THRESHOLD_LOW_MS {
            self.predictions_active = true;
        } else if rtt_ms < RTT_HYSTERESIS_MS && !self.overlay.has_cells() {
            // Only deactivate if there are no pending predictions to avoid
            // flickering during a brief RTT dip
            self.predictions_active = false;
        }
    }

    /// Whether predictions are currently underlined (RTT > 80ms).
    pub fn is_flagging(&self) -> bool {
        self.rtt_ms > RTT_THRESHOLD_HIGH_MS
    }

    /// Whether the prediction engine is currently active.
    pub fn is_active(&self) -> bool {
        self.predictions_active
    }

    /// Update the terminal dimensions. Clamps cursor predictions to the new size.
    pub fn set_terminal_size(&mut self, cols: u16, rows: u16) {
        self.terminal_cols = cols;
        self.terminal_rows = rows;
        // A resize invalidates all predictions — the server will redraw everything
        self.reset();
    }

    /// Update the confirmed cursor position from the server's last known state.
    ///
    /// Call this after `process_server_output()` has been called and the
    /// `alacritty_terminal::Term` has been updated.
    pub fn sync_confirmed_cursor(&mut self, col: u16, row: u16) {
        self.confirmed_cursor_col = col;
        self.confirmed_cursor_row = row;
        // If no predictions are pending, snap predicted cursor to confirmed
        if self.overlay.is_empty() {
            self.cursor_col = col;
            self.cursor_row = row;
        }
    }

    /// Process a keystroke and update the overlay.
    ///
    /// Returns a `PredictionAction` describing what the engine did. The caller
    /// can also call `overlay()` afterwards to get the full current overlay.
    ///
    /// Returns `PredictionAction::Inactive` if RTT is below the activation
    /// threshold and predictions are not currently active.
    pub fn process_input(&mut self, key: &KeyEvent) -> PredictionAction {
        let class = classify(key);

        // Bulk paste always resets, regardless of active state
        if class == InputClass::Bulk {
            self.reset();
            return PredictionAction::Reset;
        }

        if !self.predictions_active {
            // Still track uncertain state even when inactive, so that if
            // predictions become active mid-session, they start correctly
            if class == InputClass::Uncertain {
                self.epochs.become_tentative();
            }
            return PredictionAction::Inactive;
        }

        match class {
            InputClass::Predictable(ch) => self.predict_char(ch),
            InputClass::Backspace => self.predict_backspace(),
            InputClass::ArrowLeft => self.predict_arrow_left(),
            InputClass::ArrowRight => self.predict_arrow_right(),
            InputClass::Uncertain => {
                self.epochs.become_tentative();
                PredictionAction::BecameTentative
            }
            InputClass::Bulk => unreachable!("handled above"),
        }
    }

    /// Feed confirmed server output bytes into the engine.
    ///
    /// This compares server output against our pending predictions:
    /// - Matching bytes: remove from the prediction buffer; cells are confirmed
    ///   and removed from the overlay. If there are tentative predictions, advance
    ///   `confirmed_epoch` so they become visible.
    /// - Mismatch: kill the epoch and wipe all pending predictions from the overlay.
    ///
    /// Control sequences in the server output are ignored during comparison — we
    /// only compare printable bytes (0x20–0x7e).
    pub fn process_server_output(&mut self, bytes: &[u8]) {
        if self.predicted_bytes.is_empty() {
            return;
        }

        // Collect only the printable bytes from the server output for comparison.
        let printable_server_bytes: Vec<u8> = bytes
            .iter()
            .copied()
            .filter(|&b| b >= 0x20 && b <= 0x7e)
            .collect();

        if printable_server_bytes.is_empty() {
            return;
        }

        // Compare against predicted bytes, one at a time.
        let mut matched = 0;
        let mut mismatch = false;

        for &server_byte in &printable_server_bytes {
            if matched >= self.predicted_bytes.len() {
                break;
            }
            if self.predicted_bytes[matched] == server_byte {
                matched += 1;
            } else {
                mismatch = true;
                break;
            }
        }

        if mismatch {
            // Server output disagrees with our prediction — wipe all predictions.
            let stale_epoch = self.epochs.prediction_epoch();
            self.epochs.kill_epoch(stale_epoch);
            self.overlay.clear();
            self.predicted_bytes.clear();
            // Snap predicted cursor back to confirmed position
            self.cursor_col = self.confirmed_cursor_col;
            self.cursor_row = self.confirmed_cursor_row;
        } else if matched > 0 {
            // Server confirmed `matched` bytes of predictions.
            // Remove confirmed cells from the overlay (server is the truth now).
            // We know predictions were placed starting from the confirmed cursor,
            // advancing one column per predicted char.
            let start_col = self.confirmed_cursor_col;
            for i in 0..matched as u16 {
                self.overlay.remove(start_col + i, self.confirmed_cursor_row);
            }

            // Remove from the prediction buffer.
            self.predicted_bytes.drain(..matched);

            // Advance confirmed_epoch for each tentative prediction confirmed.
            // This makes any tentative cells visible (those typed after an uncertain input).
            for _ in 0..matched {
                self.epochs.advance_confirmed();
            }
        }
    }

    /// Get the current overlay to paint on top of the confirmed terminal state.
    ///
    /// The overlay contains all predicted cells. Callers should use
    /// `overlay.visible_cells(confirmed_epoch)` or call `overlay()` and then
    /// iterate with the engine's `confirmed_epoch()`.
    pub fn overlay(&self) -> &FrameOverlay {
        &self.overlay
    }

    /// The current confirmed epoch — pass this to `overlay.visible_cells()`.
    pub fn confirmed_epoch(&self) -> u64 {
        self.epochs.confirmed_epoch()
    }

    /// The current prediction epoch.
    pub fn prediction_epoch(&self) -> u64 {
        self.epochs.prediction_epoch()
    }

    /// The predicted cursor position.
    ///
    /// Returns `(col, row)`. If no cursor prediction is active, returns
    /// `(confirmed_cursor_col, confirmed_cursor_row)`.
    pub fn predicted_cursor(&self) -> (u16, u16) {
        if let Some(cursor) = self.overlay.cursor(self.epochs.confirmed_epoch()) {
            (cursor.col, cursor.row)
        } else {
            (self.confirmed_cursor_col, self.confirmed_cursor_row)
        }
    }

    /// Reset all prediction state.
    ///
    /// Called on paste (>100 bytes), terminal resize, or any other event
    /// that makes all current predictions invalid.
    pub fn reset(&mut self) {
        self.epochs.reset();
        self.overlay.clear();
        self.predicted_bytes.clear();
        self.cursor_col = self.confirmed_cursor_col;
        self.cursor_row = self.confirmed_cursor_row;
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    /// Predict the insertion of a printable character at the current cursor.
    fn predict_char(&mut self, ch: char) -> PredictionAction {
        let col = self.cursor_col;
        let row = self.cursor_row;
        let epoch = self.epochs.prediction_epoch();
        let underlined = self.is_flagging();

        let cell = OverlayCell::new(ch, underlined, epoch);
        self.overlay.insert(col, row, cell);

        // Update the cursor prediction
        let new_col = self.cursor_col.saturating_add(1).min(self.terminal_cols.saturating_sub(1));

        // At the right edge of the terminal, become tentative — we don't predict
        // line wrap behavior (same conservative stance as Mosh)
        if self.cursor_col >= self.terminal_cols.saturating_sub(1) {
            self.epochs.become_tentative();
        }

        self.cursor_col = new_col;

        // Update the cursor overlay
        let cursor_epoch = self.epochs.prediction_epoch();
        self.overlay.set_cursor(PredictedCursor::new(self.cursor_col, self.cursor_row, cursor_epoch));

        // Track the predicted byte for server comparison
        self.predicted_bytes.push(ch as u8);

        PredictionAction::CharInserted { col, row, ch }
    }

    /// Predict a backspace — remove the last predicted character.
    fn predict_backspace(&mut self) -> PredictionAction {
        // Become tentative for the backspace itself — the server's response
        // may differ (e.g., bash may clear more than one char with readline bindings)
        self.epochs.become_tentative();

        // Remove the last predicted cell
        self.overlay.pop_last_prediction();

        // Remove the last predicted byte
        self.predicted_bytes.pop();

        // Move cursor prediction left (don't go past column 0)
        self.cursor_col = self.cursor_col.saturating_sub(1);

        // Cursor position is shown at confirmed_epoch (immediately visible).
        // Cell content predictions use prediction_epoch (gated until confirmed),
        // but cursor position alone is low-risk to show immediately.
        let cursor_epoch = self.epochs.confirmed_epoch();
        self.overlay.set_cursor(PredictedCursor::new(self.cursor_col, self.cursor_row, cursor_epoch));

        PredictionAction::CursorMovedLeft
    }

    /// Predict left-arrow cursor movement.
    fn predict_arrow_left(&mut self) -> PredictionAction {
        self.epochs.become_tentative();
        self.cursor_col = self.cursor_col.saturating_sub(1);

        // Cursor position shown immediately (confirmed_epoch), not gated on tentative.
        let cursor_epoch = self.epochs.confirmed_epoch();
        self.overlay.set_cursor(PredictedCursor::new(self.cursor_col, self.cursor_row, cursor_epoch));

        PredictionAction::CursorMovedLeft
    }

    /// Predict right-arrow cursor movement.
    fn predict_arrow_right(&mut self) -> PredictionAction {
        self.epochs.become_tentative();
        self.cursor_col = self.cursor_col.saturating_add(1).min(self.terminal_cols.saturating_sub(1));

        // Cursor position shown immediately (confirmed_epoch), not gated on tentative.
        let cursor_epoch = self.epochs.confirmed_epoch();
        self.overlay.set_cursor(PredictedCursor::new(self.cursor_col, self.cursor_row, cursor_epoch));

        PredictionAction::CursorMovedRight
    }
}

impl Default for PredictionEngine {
    fn default() -> Self {
        Self::new(80, 24)
    }
}
