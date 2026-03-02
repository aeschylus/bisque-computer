//! Input classification for the Mosh-style prediction engine.
//!
//! This module classifies keystrokes into three categories that determine
//! what the prediction engine will do:
//!
//! - `Predictable(char)` — printable ASCII or cursor-movement arrow keys.
//!   The engine will speculatively insert the character or move the cursor.
//!
//! - `Uncertain` — Ctrl+anything, CR, ESC, backspace, and any non-ASCII input.
//!   The engine calls `become_tentative()`, hiding subsequent predictions until
//!   the server confirms the resulting state.
//!
//! - `Bulk` — paste or other large byte sequences (>100 bytes).
//!   The engine calls `reset()`, wiping all predictions entirely.
//!
//! - `Backspace` — a special case of Uncertain with defined cursor behavior:
//!   the cursor prediction moves one column left and the prediction buffer
//!   is trimmed accordingly.
//!
//! - `ArrowLeft` / `ArrowRight` — cursor movement without cell-content prediction.
//!
//! ## Design note
//!
//! `classify` is a pure function — it takes a `KeyEvent` and returns an
//! `InputClass` with no side effects. All state mutation happens in
//! `PredictionEngine` after classification.

/// A key event fed into the prediction engine.
///
/// This is a simplified representation: we only need to know what bytes
/// will be sent to the PTY, and whether Ctrl is held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    /// The bytes that will be sent to the PTY for this key press.
    /// For printable ASCII, this is a single byte. For escape sequences,
    /// this is the full sequence (e.g., `[27, b'[', b'D']` for left arrow).
    pub bytes: Vec<u8>,

    /// True if the Ctrl modifier was held when this key was pressed.
    pub ctrl_held: bool,
}

impl KeyEvent {
    /// Create a KeyEvent from a single printable ASCII character.
    pub fn printable(ch: char) -> Self {
        debug_assert!(ch.is_ascii() && ch as u8 >= 0x20 && ch as u8 <= 0x7e);
        Self {
            bytes: vec![ch as u8],
            ctrl_held: false,
        }
    }

    /// Create a KeyEvent from raw bytes (e.g., an escape sequence).
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            ctrl_held: false,
        }
    }

    /// Create a Ctrl+key event.
    pub fn ctrl(ch: char) -> Self {
        let ctrl_byte = (ch.to_ascii_uppercase() as u8).wrapping_sub(b'@');
        Self {
            bytes: vec![ctrl_byte],
            ctrl_held: true,
        }
    }

    /// Create a backspace event (sends 0x7f, DEL).
    pub fn backspace() -> Self {
        Self {
            bytes: vec![0x7f],
            ctrl_held: false,
        }
    }

    /// Create a carriage return event.
    pub fn carriage_return() -> Self {
        Self {
            bytes: vec![0x0d],
            ctrl_held: false,
        }
    }

    /// Create a left-arrow key event (CSI D).
    pub fn arrow_left() -> Self {
        Self {
            bytes: vec![0x1b, b'[', b'D'],
            ctrl_held: false,
        }
    }

    /// Create a right-arrow key event (CSI C).
    pub fn arrow_right() -> Self {
        Self {
            bytes: vec![0x1b, b'[', b'C'],
            ctrl_held: false,
        }
    }

    /// Create a bulk paste event (>100 bytes).
    pub fn bulk_paste(bytes: Vec<u8>) -> Self {
        debug_assert!(bytes.len() > 100);
        Self {
            bytes,
            ctrl_held: false,
        }
    }
}

/// The classification result for a single keystroke.
///
/// This is the output of `classify()` — a pure description of what the
/// prediction engine should do, without any mutation having occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputClass {
    /// A printable ASCII character that can be speculatively inserted.
    ///
    /// The character is in the range 0x20–0x7e (space through tilde).
    Predictable(char),

    /// Left arrow key (CSI D) — move cursor prediction one column left.
    ArrowLeft,

    /// Right arrow key (CSI C) — move cursor prediction one column right.
    ArrowRight,

    /// Backspace (DEL, 0x7f) — move cursor prediction one column left
    /// and trim the last predicted character.
    Backspace,

    /// An uncertain input that may change terminal state in ways we cannot predict.
    ///
    /// Triggers `become_tentative()` — new predictions will be hidden until
    /// the server confirms the resulting state.
    Uncertain,

    /// A bulk paste (>100 bytes).
    ///
    /// Triggers a full `reset()` — all predictions are wiped.
    Bulk,
}

/// Classify a key event into an `InputClass`.
///
/// This is a pure function: no state is read or mutated.
///
/// Classification logic (in priority order):
/// 1. Bulk paste (>100 bytes) → `Bulk`
/// 2. Left arrow CSI D → `ArrowLeft`
/// 3. Right arrow CSI C → `ArrowRight`
/// 4. Backspace (0x7f) → `Backspace`
/// 5. Carriage return (0x0d) → `Uncertain`
/// 6. ESC or any escape sequence → `Uncertain`
/// 7. Ctrl held → `Uncertain`
/// 8. Single printable ASCII (0x20–0x7e) → `Predictable(char)`
/// 9. Everything else → `Uncertain`
pub fn classify(key: &KeyEvent) -> InputClass {
    // Rule 1: bulk paste — full reset
    if key.bytes.len() > 100 {
        return InputClass::Bulk;
    }

    // Rule 2/3: arrow key escape sequences
    // CSI D = [ESC, '[', 'D'] = left arrow
    // CSI C = [ESC, '[', 'C'] = right arrow
    if key.bytes == [0x1b, b'[', b'D'] {
        return InputClass::ArrowLeft;
    }
    if key.bytes == [0x1b, b'[', b'C'] {
        return InputClass::ArrowRight;
    }

    // Rule 4: backspace (DEL)
    if key.bytes == [0x7f] {
        return InputClass::Backspace;
    }

    // Rule 5: carriage return
    if key.bytes == [0x0d] || key.bytes == [b'\n'] {
        return InputClass::Uncertain;
    }

    // Rule 6: any escape sequence (starts with 0x1b)
    if key.bytes.first() == Some(&0x1b) {
        return InputClass::Uncertain;
    }

    // Rule 7: ctrl modifier
    if key.ctrl_held {
        return InputClass::Uncertain;
    }

    // Rule 8: single printable ASCII
    if key.bytes.len() == 1 {
        let byte = key.bytes[0];
        if byte >= 0x20 && byte <= 0x7e {
            return InputClass::Predictable(byte as char);
        }

        // Control characters without Ctrl flag (e.g., raw 0x01)
        return InputClass::Uncertain;
    }

    // Rule 9: multi-byte non-ASCII (e.g., UTF-8 encoded Unicode)
    // We do not attempt to predict wide characters, emoji, or CJK input.
    InputClass::Uncertain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_ascii_is_predictable() {
        let event = KeyEvent::printable('a');
        assert_eq!(classify(&event), InputClass::Predictable('a'));
    }

    #[test]
    fn space_is_predictable() {
        let event = KeyEvent::printable(' ');
        assert_eq!(classify(&event), InputClass::Predictable(' '));
    }

    #[test]
    fn tilde_is_predictable() {
        // 0x7e is the highest printable ASCII char
        let event = KeyEvent {
            bytes: vec![0x7e],
            ctrl_held: false,
        };
        assert_eq!(classify(&event), InputClass::Predictable('~'));
    }

    #[test]
    fn backspace_classified_correctly() {
        let event = KeyEvent::backspace();
        assert_eq!(classify(&event), InputClass::Backspace);
    }

    #[test]
    fn carriage_return_is_uncertain() {
        let event = KeyEvent::carriage_return();
        assert_eq!(classify(&event), InputClass::Uncertain);
    }

    #[test]
    fn ctrl_c_is_uncertain() {
        let event = KeyEvent::ctrl('C');
        assert_eq!(classify(&event), InputClass::Uncertain);
    }

    #[test]
    fn ctrl_u_is_uncertain() {
        let event = KeyEvent::ctrl('U');
        assert_eq!(classify(&event), InputClass::Uncertain);
    }

    #[test]
    fn left_arrow_classified() {
        let event = KeyEvent::arrow_left();
        assert_eq!(classify(&event), InputClass::ArrowLeft);
    }

    #[test]
    fn right_arrow_classified() {
        let event = KeyEvent::arrow_right();
        assert_eq!(classify(&event), InputClass::ArrowRight);
    }

    #[test]
    fn escape_alone_is_uncertain() {
        let event = KeyEvent::from_bytes(vec![0x1b]);
        assert_eq!(classify(&event), InputClass::Uncertain);
    }

    #[test]
    fn escape_sequence_other_than_arrows_is_uncertain() {
        // CSI A = up arrow
        let event = KeyEvent::from_bytes(vec![0x1b, b'[', b'A']);
        assert_eq!(classify(&event), InputClass::Uncertain);
    }

    #[test]
    fn bulk_paste_classified() {
        let bytes = vec![b'a'; 101];
        let event = KeyEvent::bulk_paste(bytes);
        assert_eq!(classify(&event), InputClass::Bulk);
    }

    #[test]
    fn exactly_100_bytes_not_bulk() {
        // 100 bytes is not > 100, so it is not Bulk
        let bytes = vec![b'a'; 100];
        let event = KeyEvent::from_bytes(bytes);
        // 100 bytes is multi-byte non-ASCII-single-char, so Uncertain
        assert_eq!(classify(&event), InputClass::Uncertain);
    }

    #[test]
    fn utf8_multibyte_is_uncertain() {
        // UTF-8 for '©' is [0xC2, 0xA9]
        let event = KeyEvent::from_bytes(vec![0xC2, 0xA9]);
        assert_eq!(classify(&event), InputClass::Uncertain);
    }

    #[test]
    fn raw_control_byte_without_ctrl_flag_is_uncertain() {
        // 0x01 = Ctrl+A but sent without the flag — still uncertain
        let event = KeyEvent {
            bytes: vec![0x01],
            ctrl_held: false,
        };
        assert_eq!(classify(&event), InputClass::Uncertain);
    }

    #[test]
    fn newline_is_uncertain() {
        let event = KeyEvent::from_bytes(vec![b'\n']);
        assert_eq!(classify(&event), InputClass::Uncertain);
    }
}
