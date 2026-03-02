//! Comprehensive unit tests for the Mosh-style prediction engine.
//!
//! Tests are organized by the behavior they verify:
//!
//! 1. Basic prediction (type char → overlay shows char at cursor)
//! 2. Epoch confirmation (server echoes → confirmed_epoch advances)
//! 3. Misprediction detection (server disagrees → prediction wiped)
//! 4. Uncertain input gating (Ctrl+C → tentative, no new visible predictions)
//! 5. Backspace prediction (trim last predicted char)
//! 6. RTT gating (inactive <20ms, active ≥20ms, underlined >80ms)
//! 7. Bulk paste reset (>100 bytes → full reset)
//! 8. Cursor movement prediction (left/right arrows)
//! 9. Epoch overflow edge cases (u64::MAX behavior)
//! 10. Multi-character sequences and confirmation flow

use super::{
    InputClass, KeyEvent, PredictionAction, PredictionEngine,
    classify,
    epoch::EpochTracker,
    overlay::{FrameOverlay, OverlayCell, PredictedCursor},
};

// ── Helper constructors ────────────────────────────────────────────────────────

/// Build an engine with predictions active (RTT = 50ms).
fn active_engine() -> PredictionEngine {
    let mut engine = PredictionEngine::new(80, 24);
    engine.set_rtt(50);
    engine
}

/// Build an engine with flagging active (RTT = 100ms > 80ms threshold).
fn flagging_engine() -> PredictionEngine {
    let mut engine = PredictionEngine::new(80, 24);
    engine.set_rtt(100);
    engine
}

/// Count visible cells in the overlay at the engine's current confirmed epoch.
fn visible_cell_count(engine: &PredictionEngine) -> usize {
    engine
        .overlay()
        .visible_cells(engine.confirmed_epoch())
        .count()
}

/// Get the visible character at a position, or None.
fn visible_char_at(engine: &PredictionEngine, col: u16, row: u16) -> Option<char> {
    engine
        .overlay()
        .visible_cells(engine.confirmed_epoch())
        .find(|((c, r), _)| *c == col && *r == row)
        .map(|(_, cell)| cell.ch)
}

// ── Section 1: Basic prediction ───────────────────────────────────────────────

#[test]
fn basic_prediction_type_a_shows_at_cursor() {
    let mut engine = active_engine();
    // Initial cursor is at (0, 0)
    // Type 'a' — prediction should appear at col 0, row 0
    let action = engine.process_input(&KeyEvent::printable('a'));

    assert_eq!(
        action,
        PredictionAction::CharInserted {
            col: 0,
            row: 0,
            ch: 'a'
        }
    );
    assert_eq!(visible_char_at(&engine, 0, 0), Some('a'));
    assert_eq!(visible_cell_count(&engine), 1);
}

#[test]
fn typing_multiple_chars_advances_cursor() {
    let mut engine = active_engine();

    engine.process_input(&KeyEvent::printable('a'));
    engine.process_input(&KeyEvent::printable('b'));
    engine.process_input(&KeyEvent::printable('c'));

    // Three cells predicted at columns 0, 1, 2
    assert_eq!(visible_char_at(&engine, 0, 0), Some('a'));
    assert_eq!(visible_char_at(&engine, 1, 0), Some('b'));
    assert_eq!(visible_char_at(&engine, 2, 0), Some('c'));

    // Predicted cursor should now be at column 3
    let (cursor_col, _cursor_row) = engine.predicted_cursor();
    assert_eq!(cursor_col, 3);
}

#[test]
fn prediction_cursor_moves_after_insert() {
    let mut engine = active_engine();
    engine.process_input(&KeyEvent::printable('x'));

    let (col, row) = engine.predicted_cursor();
    assert_eq!(col, 1);
    assert_eq!(row, 0);
}

#[test]
fn predictions_visible_at_epoch_zero() {
    let mut engine = active_engine();
    // At the start, confirmed_epoch = 0, prediction_epoch = 0.
    // A prediction tagged with epoch 0 is immediately visible.
    engine.process_input(&KeyEvent::printable('h'));
    assert_eq!(visible_cell_count(&engine), 1);
}

// ── Section 2: Epoch confirmation ─────────────────────────────────────────────

#[test]
fn server_echo_removes_cell_from_overlay() {
    let mut engine = active_engine();
    engine.process_input(&KeyEvent::printable('a'));

    // Cell exists in overlay before echo
    assert!(engine.overlay().get(0, 0).is_some());

    engine.process_server_output(b"a");

    // Server confirmation removes the cell from the overlay
    assert!(
        engine.overlay().get(0, 0).is_none(),
        "Confirmed cell should be removed from overlay"
    );
}

#[test]
fn server_echo_of_tentative_prediction_advances_confirmed_epoch() {
    let mut engine = active_engine();

    // Create tentative predictions by typing Ctrl+C then chars
    engine.process_input(&KeyEvent::ctrl('C')); // prediction_epoch = 1
    engine.process_input(&KeyEvent::printable('a')); // tagged with epoch 1

    let confirmed_before = engine.confirmed_epoch(); // = 0

    // Server echoes the char — should advance confirmed_epoch
    engine.process_server_output(b"a");

    assert!(
        engine.confirmed_epoch() > confirmed_before,
        "confirmed_epoch should advance when tentative prediction is confirmed"
    );
}

#[test]
fn server_echo_of_non_tentative_does_not_change_confirmed_epoch() {
    let mut engine = active_engine();
    // No uncertain input → both epochs stay at 0
    engine.process_input(&KeyEvent::printable('a'));

    let confirmed_before = engine.confirmed_epoch(); // = 0
    engine.process_server_output(b"a");

    // Both epochs were 0, advance_confirmed is a no-op when confirmed == prediction
    assert_eq!(engine.confirmed_epoch(), confirmed_before);
}

#[test]
fn server_echo_of_multiple_predicted_chars_clears_overlay() {
    let mut engine = active_engine();
    engine.process_input(&KeyEvent::printable('a'));
    engine.process_input(&KeyEvent::printable('b'));

    // Server echoes both characters
    engine.process_server_output(b"ab");

    // Confirmed cells removed from overlay
    assert!(engine.overlay().get(0, 0).is_none());
    assert!(engine.overlay().get(1, 0).is_none());
}

#[test]
fn server_output_no_predictions_is_no_op() {
    let mut engine = active_engine();
    // No predictions made — process_server_output should not change anything
    let before = engine.confirmed_epoch();
    engine.process_server_output(b"anything");
    assert_eq!(engine.confirmed_epoch(), before);
}

// ── Section 3: Misprediction detection ───────────────────────────────────────

#[test]
fn misprediction_wipes_overlay() {
    let mut engine = active_engine();
    engine.process_input(&KeyEvent::printable('a'));

    // Server sends a different character
    engine.process_server_output(b"b");

    // After misprediction, overlay should be empty
    assert_eq!(engine.overlay().cell_count(), 0);
}

#[test]
fn misprediction_resets_cursor_to_confirmed() {
    let mut engine = active_engine();
    engine.sync_confirmed_cursor(5, 2);
    // Now type a character (moves predicted cursor to col 6)
    engine.process_input(&KeyEvent::printable('z'));

    // Server sends something different
    engine.process_server_output(b"q");

    // Cursor snaps back to confirmed position
    let (col, row) = engine.predicted_cursor();
    assert_eq!(col, 5);
    assert_eq!(row, 2);
}

#[test]
fn misprediction_kills_epoch() {
    let mut engine = active_engine();
    engine.process_input(&KeyEvent::printable('a'));

    let epoch_before = engine.prediction_epoch();
    engine.process_server_output(b"b"); // mismatch

    // After kill_epoch, prediction_epoch is higher
    assert!(engine.prediction_epoch() > epoch_before);
    // And confirmed_epoch has caught up (no permanent gap)
    assert_eq!(engine.confirmed_epoch(), engine.prediction_epoch());
}

// ── Section 4: Uncertain input gating ────────────────────────────────────────

#[test]
fn ctrl_c_becomes_tentative() {
    let mut engine = active_engine();
    let epoch_before = engine.prediction_epoch();

    let action = engine.process_input(&KeyEvent::ctrl('C'));

    assert_eq!(action, PredictionAction::BecameTentative);
    assert!(engine.prediction_epoch() > epoch_before);
}

#[test]
fn predictions_after_ctrl_c_are_hidden_until_server_confirms() {
    let mut engine = active_engine();

    // Type 'a' (visible prediction, epoch 0)
    engine.process_input(&KeyEvent::printable('a'));
    assert_eq!(visible_cell_count(&engine), 1);

    // Ctrl+C → prediction_epoch becomes 1, confirmed_epoch still 0
    engine.process_input(&KeyEvent::ctrl('C'));

    // Type 'b' → tagged with epoch 1, hidden since confirmed_epoch = 0 < 1
    engine.process_input(&KeyEvent::printable('b'));

    // 'a' is at epoch 0 (visible), 'b' is at epoch 1 (hidden)
    let visible_chars: Vec<char> = engine
        .overlay()
        .visible_cells(engine.confirmed_epoch())
        .map(|(_, cell)| cell.ch)
        .collect();

    // 'b' should be invisible since the epoch hasn't been confirmed
    assert!(!visible_chars.contains(&'b'));
}

#[test]
fn esc_key_becomes_tentative() {
    let mut engine = active_engine();
    let epoch_before = engine.prediction_epoch();

    engine.process_input(&KeyEvent::from_bytes(vec![0x1b]));

    assert!(engine.prediction_epoch() > epoch_before);
}

#[test]
fn carriage_return_becomes_tentative() {
    let mut engine = active_engine();
    let epoch_before = engine.prediction_epoch();

    let action = engine.process_input(&KeyEvent::carriage_return());

    assert_eq!(action, PredictionAction::BecameTentative);
    assert!(engine.prediction_epoch() > epoch_before);
}

#[test]
fn no_new_visible_predictions_while_tentative() {
    let mut engine = active_engine();

    // Trigger tentative state via Ctrl+U
    engine.process_input(&KeyEvent::ctrl('U'));
    // confirmed_epoch < prediction_epoch means tentative predictions exist
    assert!(engine.confirmed_epoch() < engine.prediction_epoch());

    // Type characters in tentative state
    engine.process_input(&KeyEvent::printable('x'));
    engine.process_input(&KeyEvent::printable('y'));

    // Those new cells are invisible (they are tagged with the tentative epoch)
    let visible = visible_cell_count(&engine);
    assert_eq!(visible, 0, "Tentative cells should not be visible");
}

// ── Section 5: Backspace prediction ──────────────────────────────────────────

#[test]
fn backspace_removes_last_predicted_char() {
    let mut engine = active_engine();

    engine.process_input(&KeyEvent::printable('a'));
    engine.process_input(&KeyEvent::printable('b'));
    engine.process_input(&KeyEvent::printable('c'));

    // Backspace should remove 'c' at col 2
    let action = engine.process_input(&KeyEvent::backspace());
    assert_eq!(action, PredictionAction::CursorMovedLeft);

    // 'a' and 'b' still in overlay, 'c' removed
    assert!(engine.overlay().get(0, 0).map(|c| c.ch) == Some('a'));
    assert!(engine.overlay().get(1, 0).map(|c| c.ch) == Some('b'));
    assert!(engine.overlay().get(2, 0).is_none());
}

#[test]
fn backspace_moves_cursor_left() {
    let mut engine = active_engine();

    engine.process_input(&KeyEvent::printable('a'));
    engine.process_input(&KeyEvent::printable('b'));
    // cursor is at col 2

    engine.process_input(&KeyEvent::backspace());
    // cursor should be at col 1

    let (col, _) = engine.predicted_cursor();
    assert_eq!(col, 1);
}

#[test]
fn backspace_at_col_zero_does_not_underflow() {
    let mut engine = active_engine();
    // cursor starts at (0, 0)
    engine.process_input(&KeyEvent::backspace());

    let (col, _) = engine.predicted_cursor();
    assert_eq!(col, 0, "Cursor should clamp to column 0");
}

#[test]
fn type_abc_backspace_shows_ab() {
    let mut engine = active_engine();

    engine.process_input(&KeyEvent::printable('a'));
    engine.process_input(&KeyEvent::printable('b'));
    engine.process_input(&KeyEvent::printable('c'));
    engine.process_input(&KeyEvent::backspace());

    // Only 'a' at col 0 and 'b' at col 1 should be in overlay
    let visible: Vec<_> = engine
        .overlay()
        .visible_cells(engine.confirmed_epoch())
        .map(|((col, _), cell)| (col, cell.ch))
        .collect();

    // 'c' should be gone
    assert!(!visible.iter().any(|(_, ch)| *ch == 'c'));
    // 'a' and 'b' should remain
    assert!(visible.iter().any(|(_, ch)| *ch == 'a'));
    assert!(visible.iter().any(|(_, ch)| *ch == 'b'));
}

// ── Section 6: RTT gating ─────────────────────────────────────────────────────

#[test]
fn rtt_below_20ms_engine_inactive() {
    let mut engine = PredictionEngine::new(80, 24);
    engine.set_rtt(10); // Below threshold

    let action = engine.process_input(&KeyEvent::printable('a'));
    assert_eq!(action, PredictionAction::Inactive);
    assert_eq!(visible_cell_count(&engine), 0);
}

#[test]
fn rtt_at_0ms_engine_inactive() {
    let mut engine = PredictionEngine::new(80, 24);
    // RTT = 0 (default) → inactive

    let action = engine.process_input(&KeyEvent::printable('a'));
    assert_eq!(action, PredictionAction::Inactive);
}

#[test]
fn rtt_20ms_activates_predictions() {
    let mut engine = PredictionEngine::new(80, 24);
    engine.set_rtt(20);

    let action = engine.process_input(&KeyEvent::printable('a'));
    assert_eq!(
        action,
        PredictionAction::CharInserted {
            col: 0,
            row: 0,
            ch: 'a'
        }
    );
}

#[test]
fn rtt_50ms_predictions_shown_without_underline() {
    let mut engine = PredictionEngine::new(80, 24);
    engine.set_rtt(50);

    engine.process_input(&KeyEvent::printable('a'));

    let cell = engine.overlay().get(0, 0).unwrap();
    assert!(!cell.underlined, "At RTT=50ms, predictions should not be underlined");
}

#[test]
fn rtt_above_80ms_predictions_underlined() {
    let mut engine = flagging_engine();
    engine.process_input(&KeyEvent::printable('a'));

    let cell = engine.overlay().get(0, 0).unwrap();
    assert!(cell.underlined, "At RTT>80ms, predictions should be underlined (flagged)");
}

#[test]
fn rtt_exactly_80ms_not_flagging() {
    let mut engine = PredictionEngine::new(80, 24);
    engine.set_rtt(80);
    // Threshold is > 80ms, not >= 80ms
    assert!(!engine.is_flagging());

    engine.process_input(&KeyEvent::printable('a'));
    let cell = engine.overlay().get(0, 0).unwrap();
    assert!(!cell.underlined);
}

#[test]
fn rtt_81ms_is_flagging() {
    let mut engine = PredictionEngine::new(80, 24);
    engine.set_rtt(81);
    assert!(engine.is_flagging());
}

#[test]
fn rtt_drops_below_threshold_deactivates_when_no_pending() {
    let mut engine = PredictionEngine::new(80, 24);
    engine.set_rtt(50); // Activate
    assert!(engine.is_active());

    // Drop RTT with no pending predictions
    engine.set_rtt(5);
    assert!(!engine.is_active());
}

#[test]
fn rtt_drops_but_stays_active_while_predictions_pending() {
    let mut engine = PredictionEngine::new(80, 24);
    engine.set_rtt(50); // Activate
    engine.process_input(&KeyEvent::printable('a')); // Make a prediction

    // Drop RTT — should stay active since prediction is pending
    engine.set_rtt(5);
    assert!(
        engine.is_active(),
        "Should stay active while predictions are pending"
    );
}

// ── Section 7: Bulk paste reset ───────────────────────────────────────────────

#[test]
fn bulk_paste_resets_engine() {
    let mut engine = active_engine();
    engine.process_input(&KeyEvent::printable('a'));
    engine.process_input(&KeyEvent::printable('b'));

    assert!(engine.overlay().has_cells());

    let bytes = vec![b'x'; 101];
    let action = engine.process_input(&KeyEvent::bulk_paste(bytes));

    assert_eq!(action, PredictionAction::Reset);
    assert!(!engine.overlay().has_cells());
    assert_eq!(engine.overlay().cell_count(), 0);
}

#[test]
fn bulk_paste_resets_even_when_engine_inactive() {
    let mut engine = PredictionEngine::new(80, 24);
    // Engine is inactive (RTT = 0), but bulk paste should still reset

    let bytes = vec![b'a'; 101];
    let action = engine.process_input(&KeyEvent::bulk_paste(bytes));

    assert_eq!(action, PredictionAction::Reset);
}

#[test]
fn exactly_100_bytes_is_not_bulk() {
    let bytes = vec![b'a'; 100];
    let key = KeyEvent::from_bytes(bytes);
    // 100 bytes is not > 100, so classify returns Uncertain (multi-byte, non-single-ASCII)
    assert_eq!(classify(&key), InputClass::Uncertain);
}

#[test]
fn bulk_paste_resets_epochs() {
    let mut engine = active_engine();
    engine.process_input(&KeyEvent::ctrl('C')); // advance prediction_epoch
    engine.process_input(&KeyEvent::printable('x'));

    let bytes = vec![b'z'; 101];
    engine.process_input(&KeyEvent::bulk_paste(bytes));

    // After reset, both epochs should be 0
    assert_eq!(engine.prediction_epoch(), 0);
    assert_eq!(engine.confirmed_epoch(), 0);
}

// ── Section 8: Cursor movement prediction ────────────────────────────────────

#[test]
fn left_arrow_moves_cursor_prediction_left() {
    let mut engine = active_engine();
    // Move cursor to col 5 manually
    for _ in 0..5 {
        engine.process_input(&KeyEvent::printable('x'));
    }
    let (col_before, _) = engine.predicted_cursor();
    assert_eq!(col_before, 5);

    engine.process_input(&KeyEvent::arrow_left());

    let (col_after, _) = engine.predicted_cursor();
    assert_eq!(col_after, 4);
}

#[test]
fn right_arrow_moves_cursor_prediction_right() {
    let mut engine = active_engine();
    // Cursor starts at col 0
    let action = engine.process_input(&KeyEvent::arrow_right());

    assert_eq!(action, PredictionAction::CursorMovedRight);
    let (col, _) = engine.predicted_cursor();
    assert_eq!(col, 1);
}

#[test]
fn left_arrow_at_col_zero_does_not_underflow() {
    let mut engine = active_engine();
    // cursor at (0, 0)
    engine.process_input(&KeyEvent::arrow_left());

    let (col, _) = engine.predicted_cursor();
    assert_eq!(col, 0);
}

#[test]
fn right_arrow_clamps_at_terminal_width() {
    let mut engine = PredictionEngine::new(10, 5);
    engine.set_rtt(50);

    // Move cursor to the last column
    for _ in 0..15 {
        engine.process_input(&KeyEvent::arrow_right());
    }

    let (col, _) = engine.predicted_cursor();
    assert_eq!(col, 9, "Cursor should clamp to terminal_cols - 1 = 9");
}

#[test]
fn arrow_keys_become_tentative() {
    let mut engine = active_engine();
    let epoch_before = engine.prediction_epoch();

    engine.process_input(&KeyEvent::arrow_left());

    assert!(engine.prediction_epoch() > epoch_before);
}

#[test]
fn arrow_keys_do_not_predict_cell_content() {
    let mut engine = active_engine();
    engine.process_input(&KeyEvent::arrow_right());

    // No cell content should be predicted — only cursor movement
    assert_eq!(engine.overlay().cell_count(), 0);
}

// ── Section 9: Epoch overflow edge cases ─────────────────────────────────────

#[test]
fn epoch_tracker_saturating_at_u64_max() {
    let mut tracker = EpochTracker::with_epochs(u64::MAX, u64::MAX);
    // become_tentative should not panic
    tracker.become_tentative();
    assert_eq!(tracker.prediction_epoch(), u64::MAX);
}

#[test]
fn epoch_advance_confirmed_saturates() {
    let mut tracker = EpochTracker::with_epochs(u64::MAX, u64::MAX - 1);
    tracker.advance_confirmed();
    assert_eq!(tracker.confirmed_epoch(), u64::MAX);
    // Second advance should not exceed prediction_epoch
    tracker.advance_confirmed();
    assert_eq!(tracker.confirmed_epoch(), u64::MAX);
}

#[test]
fn kill_epoch_at_max_does_not_panic() {
    let mut tracker = EpochTracker::with_epochs(u64::MAX, u64::MAX);
    let result = tracker.kill_epoch(u64::MAX);
    // Should not panic; result is MAX (saturated)
    assert_eq!(result, u64::MAX);
}

#[test]
fn prediction_engine_with_max_rtt_does_not_panic() {
    let mut engine = PredictionEngine::new(80, 24);
    engine.set_rtt(u32::MAX);
    assert!(engine.is_flagging());
    // Should work normally
    let action = engine.process_input(&KeyEvent::printable('a'));
    assert!(matches!(action, PredictionAction::CharInserted { .. }));
}

// ── Section 10: Multi-character confirmation flow ─────────────────────────────

#[test]
fn multi_char_prediction_and_confirmation() {
    let mut engine = active_engine();

    engine.process_input(&KeyEvent::printable('h'));
    engine.process_input(&KeyEvent::printable('e'));
    engine.process_input(&KeyEvent::printable('l'));
    engine.process_input(&KeyEvent::printable('l'));
    engine.process_input(&KeyEvent::printable('o'));

    // 5 cells predicted
    assert_eq!(engine.overlay().cell_count(), 5);

    // Server echoes "hello"
    engine.process_server_output(b"hello");

    // All 5 cells should be removed from the overlay (confirmed by server)
    assert_eq!(
        engine.overlay().cell_count(),
        0,
        "All predicted cells should be cleared after confirmation"
    );
}

#[test]
fn partial_confirmation_clears_matched_cells() {
    let mut engine = active_engine();

    engine.process_input(&KeyEvent::printable('a'));
    engine.process_input(&KeyEvent::printable('b'));
    engine.process_input(&KeyEvent::printable('c'));

    // Server echoes only "a"
    engine.process_server_output(b"a");

    // 'a' at col 0 should be cleared
    assert!(engine.overlay().get(0, 0).is_none());
    // 'b' at col 1 and 'c' at col 2 should remain
    assert!(engine.overlay().get(1, 0).is_some());
    assert!(engine.overlay().get(2, 0).is_some());
}

#[test]
fn control_sequences_in_server_output_are_ignored() {
    let mut engine = active_engine();
    engine.process_input(&KeyEvent::printable('a'));

    // Server sends cursor movement + the echoed char
    // The control sequence bytes should be skipped during comparison
    let server_output = b"\x1b[1;1Ha";
    engine.process_server_output(server_output);

    // The 'a' should be matched — control bytes ignored
    assert!(engine.confirmed_epoch() > 0);
}

#[test]
fn type_then_ctrl_c_then_type_creates_visible_invisible_sequence() {
    let mut engine = active_engine();

    // Type 'a' at epoch 0 — visible
    engine.process_input(&KeyEvent::printable('a'));

    // Ctrl+C → epoch becomes 1, confirmed still 0
    engine.process_input(&KeyEvent::ctrl('C'));

    // Type 'b' at epoch 1 — invisible until epoch 1 is confirmed
    engine.process_input(&KeyEvent::printable('b'));

    let overlay = engine.overlay();
    let confirmed = engine.confirmed_epoch();

    // 'a' at epoch 0 is visible (0 <= confirmed=0)
    let a_cell = overlay.get(0, 0);
    assert!(a_cell.is_some());
    assert!(a_cell.unwrap().is_visible(confirmed));

    // 'b' at epoch 1 is invisible (1 > confirmed=0)
    let b_cell = overlay.get(1, 0);
    // 'b' might be at a different column because the cursor advanced
    // The key invariant: no 'b' is visible
    let any_b_visible = overlay
        .visible_cells(confirmed)
        .any(|(_, cell)| cell.ch == 'b');
    assert!(!any_b_visible, "'b' should be invisible while epoch is tentative");
}

#[test]
fn reset_clears_everything() {
    let mut engine = active_engine();

    engine.process_input(&KeyEvent::printable('a'));
    engine.process_input(&KeyEvent::printable('b'));
    engine.process_input(&KeyEvent::ctrl('C'));

    engine.reset();

    assert_eq!(engine.overlay().cell_count(), 0);
    assert_eq!(engine.prediction_epoch(), 0);
    assert_eq!(engine.confirmed_epoch(), 0);
    assert!(!engine.overlay().has_cells());
}

#[test]
fn sync_confirmed_cursor_snaps_prediction_when_no_overlay() {
    let mut engine = active_engine();

    engine.sync_confirmed_cursor(10, 3);

    let (col, row) = engine.predicted_cursor();
    assert_eq!(col, 10);
    assert_eq!(row, 3);
}

#[test]
fn sync_confirmed_cursor_does_not_snap_while_predictions_pending() {
    let mut engine = active_engine();
    engine.process_input(&KeyEvent::printable('a'));
    // predicted cursor is now at col 1

    // Server confirms cursor at col 5, but we have a pending prediction
    engine.sync_confirmed_cursor(5, 0);

    let (col, _) = engine.predicted_cursor();
    // Predicted cursor should still be at 1, not snapped to 5
    assert_eq!(col, 1, "Should not snap cursor while predictions are pending");
}

// ── Section 11: Terminal edge cases ──────────────────────────────────────────

#[test]
fn typing_at_right_edge_becomes_tentative() {
    // 10-column terminal
    let mut engine = PredictionEngine::new(10, 5);
    engine.set_rtt(50);

    // Move cursor to the last column (col 9)
    for _ in 0..9 {
        engine.process_input(&KeyEvent::printable('x'));
    }

    let epoch_before = engine.prediction_epoch();
    // Type one more char at the right edge
    engine.process_input(&KeyEvent::printable('z'));

    // Should become tentative at the right margin
    assert!(
        engine.prediction_epoch() > epoch_before,
        "Typing at right edge should become tentative"
    );
}

#[test]
fn overlay_cell_created_with_correct_epoch() {
    let mut engine = active_engine();

    // Advance to epoch 2
    engine.process_input(&KeyEvent::ctrl('C'));
    engine.process_input(&KeyEvent::ctrl('C'));
    assert_eq!(engine.prediction_epoch(), 2);

    // Type 'a' — cell should be tagged with epoch 2
    engine.process_input(&KeyEvent::printable('a'));

    let cell = engine.overlay().get(0, 0).unwrap();
    assert_eq!(cell.tentative_until_epoch, 2);
    // With confirmed_epoch = 0, this cell should be invisible
    assert!(!cell.is_visible(0));
    // But visible when epoch 2 is confirmed
    assert!(cell.is_visible(2));
}

// ── Section 12: Input classification ─────────────────────────────────────────

#[test]
fn classify_all_printable_ascii_range() {
    for byte in 0x20u8..=0x7eu8 {
        let key = KeyEvent {
            bytes: vec![byte],
            ctrl_held: false,
        };
        let class = classify(&key);
        assert_eq!(
            class,
            InputClass::Predictable(byte as char),
            "byte 0x{:02x} should be Predictable",
            byte
        );
    }
}

#[test]
fn classify_all_ctrl_chars_uncertain() {
    // Ctrl+A through Ctrl+Z
    for ch in b'A'..=b'Z' {
        let key = KeyEvent::ctrl(ch as char);
        assert_eq!(
            classify(&key),
            InputClass::Uncertain,
            "Ctrl+{} should be Uncertain",
            ch as char
        );
    }
}

#[test]
fn classify_backspace_correct() {
    assert_eq!(classify(&KeyEvent::backspace()), InputClass::Backspace);
}

#[test]
fn classify_arrows_correct() {
    assert_eq!(classify(&KeyEvent::arrow_left()), InputClass::ArrowLeft);
    assert_eq!(classify(&KeyEvent::arrow_right()), InputClass::ArrowRight);
}
