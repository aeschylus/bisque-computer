//! The frame overlay — a sparse grid of predicted cells painted over the
//! confirmed terminal state.
//!
//! ## Design
//!
//! The overlay is a `HashMap<(u16, u16), OverlayCell>` keyed by `(col, row)`.
//! This is intentionally sparse: most cells are confirmed server state. We
//! only store cells where the prediction engine has made a speculative change.
//!
//! The overlay also tracks the predicted cursor position separately from the
//! cell content, since cursor-only prediction (without cell content prediction)
//! is the safest mode of operation.
//!
//! ## Epoch filtering
//!
//! `OverlayCell` stores the `tentative_until_epoch` at which it was created.
//! The `FrameOverlay::visible_cells(confirmed_epoch)` iterator only yields
//! cells whose `tentative_until_epoch <= confirmed_epoch` — i.e., cells whose
//! epoch has been confirmed by server output.
//!
//! This means the overlay can contain many cells that are currently invisible.
//! They become visible as the server confirms output, without requiring any
//! explicit "promotion" step.

use std::collections::HashMap;

/// A single predicted cell overlay entry.
///
/// Stores the predicted character, its display attributes, and the epoch
/// at which it was created (used to gate visibility).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayCell {
    /// The predicted character to display.
    pub ch: char,

    /// Whether to underline this prediction (set when RTT > 80ms).
    ///
    /// Underlining signals to the user that this character is speculative.
    /// It matches Mosh's "flagging" behavior.
    pub underlined: bool,

    /// The epoch at which this prediction was created.
    ///
    /// This cell is only shown when `confirmed_epoch >= tentative_until_epoch`.
    pub tentative_until_epoch: u64,
}

impl OverlayCell {
    /// Create a new overlay cell.
    pub fn new(ch: char, underlined: bool, tentative_until_epoch: u64) -> Self {
        Self {
            ch,
            underlined,
            tentative_until_epoch,
        }
    }

    /// Whether this cell should be shown given the current confirmed epoch.
    pub fn is_visible(&self, confirmed_epoch: u64) -> bool {
        confirmed_epoch >= self.tentative_until_epoch
    }
}

/// The predicted cursor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredictedCursor {
    /// Column (0-indexed, from left).
    pub col: u16,

    /// Row (0-indexed, from top).
    pub row: u16,

    /// The epoch at which this cursor prediction was created.
    pub tentative_until_epoch: u64,
}

impl PredictedCursor {
    pub fn new(col: u16, row: u16, tentative_until_epoch: u64) -> Self {
        Self {
            col,
            row,
            tentative_until_epoch,
        }
    }

    pub fn is_visible(&self, confirmed_epoch: u64) -> bool {
        confirmed_epoch >= self.tentative_until_epoch
    }
}

/// A sparse overlay grid to be painted on top of the confirmed terminal state.
///
/// This struct owns all predicted cell state. It does not modify the underlying
/// `alacritty_terminal::Term` — it only describes what to paint on top.
#[derive(Debug, Clone)]
pub struct FrameOverlay {
    /// Sparse map of (col, row) → predicted cell.
    cells: HashMap<(u16, u16), OverlayCell>,

    /// The predicted cursor position, if any.
    cursor: Option<PredictedCursor>,

    /// Ordered list of predicted positions for backspace support.
    ///
    /// When the user types characters and then backspaces, we need to know
    /// which cell to remove. This stack tracks the sequence of predicted
    /// cursor positions in insertion order.
    prediction_stack: Vec<(u16, u16)>,
}

impl FrameOverlay {
    /// Create a new empty overlay.
    pub fn new() -> Self {
        Self {
            cells: HashMap::new(),
            cursor: None,
            prediction_stack: Vec::new(),
        }
    }

    /// Insert a predicted cell at the given position.
    ///
    /// If a cell already exists at that position, it is replaced.
    /// The position is also pushed onto the prediction stack for backspace support.
    pub fn insert(&mut self, col: u16, row: u16, cell: OverlayCell) {
        self.prediction_stack.push((col, row));
        self.cells.insert((col, row), cell);
    }

    /// Remove the most recently predicted cell (backspace support).
    ///
    /// Returns the position of the removed cell, or `None` if the stack is empty.
    pub fn pop_last_prediction(&mut self) -> Option<(u16, u16)> {
        while let Some(pos) = self.prediction_stack.pop() {
            if self.cells.remove(&pos).is_some() {
                return Some(pos);
            }
            // Cell was already removed (e.g., cleared by epoch kill) — keep popping
        }
        None
    }

    /// Set the predicted cursor position.
    pub fn set_cursor(&mut self, cursor: PredictedCursor) {
        self.cursor = Some(cursor);
    }

    /// Get the predicted cursor position, if visible at the given confirmed epoch.
    pub fn cursor(&self, confirmed_epoch: u64) -> Option<&PredictedCursor> {
        self.cursor
            .as_ref()
            .filter(|c| c.is_visible(confirmed_epoch))
    }

    /// Returns true if the overlay has no cells and no cursor.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() && self.cursor.is_none()
    }

    /// Returns true if there are any predicted cells (visible or not).
    pub fn has_cells(&self) -> bool {
        !self.cells.is_empty()
    }

    /// Iterate over all cells that are currently visible at the given confirmed epoch.
    ///
    /// Yields `((col, row), &OverlayCell)` tuples.
    pub fn visible_cells(
        &self,
        confirmed_epoch: u64,
    ) -> impl Iterator<Item = ((u16, u16), &OverlayCell)> {
        self.cells
            .iter()
            .filter(move |(_, cell)| cell.is_visible(confirmed_epoch))
            .map(|(pos, cell)| (*pos, cell))
    }

    /// Iterate over all cells regardless of epoch visibility.
    ///
    /// Useful for debugging and internal state inspection.
    pub fn iter(&self) -> impl Iterator<Item = ((u16, u16), &OverlayCell)> {
        self.cells.iter().map(|(pos, cell)| (*pos, cell))
    }

    /// Remove all cells whose `tentative_until_epoch > confirmed_epoch`.
    ///
    /// Called when a misprediction is detected and we need to cull stale
    /// predictions that will never be confirmed.
    pub fn cull_tentative(&mut self, confirmed_epoch: u64) {
        self.cells
            .retain(|_, cell| cell.tentative_until_epoch <= confirmed_epoch);
        // Trim the prediction stack to only positions still in the map
        self.prediction_stack
            .retain(|pos| self.cells.contains_key(pos));
    }

    /// Remove a specific cell by position. Returns the removed cell if it existed.
    pub fn remove(&mut self, col: u16, row: u16) -> Option<OverlayCell> {
        self.cells.remove(&(col, row))
    }

    /// Get a cell at a position without removing it.
    pub fn get(&self, col: u16, row: u16) -> Option<&OverlayCell> {
        self.cells.get(&(col, row))
    }

    /// Clear all predicted cells and cursor. Equivalent to a full reset.
    pub fn clear(&mut self) {
        self.cells.clear();
        self.cursor = None;
        self.prediction_stack.clear();
    }

    /// The number of cells in the overlay (including invisible/tentative ones).
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }
}

impl Default for FrameOverlay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(ch: char, epoch: u64) -> OverlayCell {
        OverlayCell::new(ch, false, epoch)
    }

    #[test]
    fn new_overlay_is_empty() {
        let overlay = FrameOverlay::new();
        assert!(overlay.is_empty());
        assert_eq!(overlay.cell_count(), 0);
    }

    #[test]
    fn insert_and_retrieve_cell() {
        let mut overlay = FrameOverlay::new();
        overlay.insert(5, 2, cell('a', 0));
        assert_eq!(overlay.cell_count(), 1);
        let c = overlay.get(5, 2).unwrap();
        assert_eq!(c.ch, 'a');
    }

    #[test]
    fn visible_cells_filters_by_confirmed_epoch() {
        let mut overlay = FrameOverlay::new();
        // epoch 0 cell — visible immediately
        overlay.insert(0, 0, cell('a', 0));
        // epoch 2 cell — only visible when confirmed >= 2
        overlay.insert(1, 0, cell('b', 2));

        let visible: Vec<_> = overlay.visible_cells(0).collect();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].1.ch, 'a');

        let visible: Vec<_> = overlay.visible_cells(2).collect();
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn pop_last_prediction_removes_most_recent() {
        let mut overlay = FrameOverlay::new();
        overlay.insert(0, 0, cell('a', 0));
        overlay.insert(1, 0, cell('b', 0));
        overlay.insert(2, 0, cell('c', 0));

        let popped = overlay.pop_last_prediction();
        assert_eq!(popped, Some((2, 0)));
        assert_eq!(overlay.cell_count(), 2);
    }

    #[test]
    fn cull_tentative_removes_future_epoch_cells() {
        let mut overlay = FrameOverlay::new();
        overlay.insert(0, 0, cell('a', 0)); // confirmed at epoch 0
        overlay.insert(1, 0, cell('b', 5)); // tentative until epoch 5

        overlay.cull_tentative(2); // confirmed_epoch = 2

        // 'a' stays (0 <= 2), 'b' is removed (5 > 2)
        assert_eq!(overlay.cell_count(), 1);
        assert!(overlay.get(0, 0).is_some());
        assert!(overlay.get(1, 0).is_none());
    }

    #[test]
    fn clear_resets_all_state() {
        let mut overlay = FrameOverlay::new();
        overlay.insert(0, 0, cell('a', 0));
        overlay.set_cursor(PredictedCursor::new(1, 0, 0));

        overlay.clear();

        assert!(overlay.is_empty());
        assert!(overlay.cursor(0).is_none());
    }

    #[test]
    fn cursor_visibility_gated_by_epoch() {
        let mut overlay = FrameOverlay::new();
        overlay.set_cursor(PredictedCursor::new(3, 1, 2));

        // Not visible at epoch 1
        assert!(overlay.cursor(1).is_none());
        // Visible at epoch 2
        assert!(overlay.cursor(2).is_some());
    }

    #[test]
    fn overlay_cell_underlined_flag() {
        let cell = OverlayCell::new('x', true, 0);
        assert!(cell.underlined);
    }
}
