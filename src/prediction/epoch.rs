//! Epoch tracking for the Mosh-style prediction engine.
//!
//! The epoch system is the core concurrency-safety mechanism that prevents
//! the prediction engine from showing predictions whose underlying state has
//! been invalidated by unconfirmed control-key input.
//!
//! ## How epochs work
//!
//! `prediction_epoch` is a monotonically increasing counter. It is incremented
//! every time the user types something "uncertain" — a Ctrl key, CR, ESC, or
//! any other input whose terminal effect we cannot safely predict (e.g. Ctrl+C
//! might kill the foreground process and change the terminal state completely).
//!
//! Every prediction is tagged with a `tentative_until_epoch` value equal to
//! `prediction_epoch` at the time it was created. A prediction is only *shown*
//! when `confirmed_epoch >= tentative_until_epoch`.
//!
//! `confirmed_epoch` advances when server output is received that matches a
//! prediction — i.e., the server has "caught up" to a known state.
//!
//! The result: after a Ctrl+C, new predictions become tentative (invisible)
//! until the server sends output that advances `confirmed_epoch` past the
//! epoch that was current when Ctrl+C was typed.

/// Tracks the prediction/confirmation epoch pair.
///
/// This is a pure-data type with no hidden state. All mutation is explicit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochTracker {
    /// Monotonically increasing counter. Incremented on uncertain input.
    /// New predictions are tagged `tentative_until = prediction_epoch`.
    prediction_epoch: u64,

    /// Advances as server output confirms our predictions.
    /// A prediction tagged `tentative_until_epoch = N` is visible only when
    /// `confirmed_epoch >= N`.
    confirmed_epoch: u64,
}

impl EpochTracker {
    /// Create a new EpochTracker with both epochs at zero.
    pub fn new() -> Self {
        Self {
            prediction_epoch: 0,
            confirmed_epoch: 0,
        }
    }

    /// The current prediction epoch. New predictions are tagged with this value.
    pub fn prediction_epoch(&self) -> u64 {
        self.prediction_epoch
    }

    /// The current confirmed epoch. Predictions tagged <= this are visible.
    pub fn confirmed_epoch(&self) -> u64 {
        self.confirmed_epoch
    }

    /// Increment the prediction epoch due to uncertain input (Ctrl key, CR, ESC).
    ///
    /// After this call, any new predictions will be invisible until the server
    /// sends output that advances `confirmed_epoch` past the new value.
    pub fn become_tentative(&mut self) {
        self.prediction_epoch = self.prediction_epoch.saturating_add(1);
    }

    /// Advance the confirmed epoch, making tentative predictions visible.
    ///
    /// Called when server output is received that matches a prediction,
    /// confirming that the server has processed input up to a certain point.
    ///
    /// The confirmed epoch never exceeds the prediction epoch — we cannot
    /// confirm epochs we haven't predicted yet.
    pub fn advance_confirmed(&mut self) {
        if self.confirmed_epoch < self.prediction_epoch {
            self.confirmed_epoch = self.confirmed_epoch.saturating_add(1);
        }
    }

    /// Check whether a prediction tagged with the given epoch is currently visible.
    ///
    /// A prediction is visible when `confirmed_epoch >= tentative_until_epoch`.
    pub fn is_confirmed(&self, tentative_until_epoch: u64) -> bool {
        self.confirmed_epoch >= tentative_until_epoch
    }

    /// Check whether any predictions are currently in a tentative (hidden) state.
    ///
    /// Returns true if the confirmed epoch is behind the prediction epoch,
    /// meaning some recent predictions are not yet visible.
    pub fn has_tentative_predictions(&self) -> bool {
        self.confirmed_epoch < self.prediction_epoch
    }

    /// Create an EpochTracker with specific epoch values.
    ///
    /// Used for testing overflow/boundary conditions.
    #[cfg(test)]
    pub fn with_epochs(prediction_epoch: u64, confirmed_epoch: u64) -> Self {
        Self {
            prediction_epoch,
            confirmed_epoch,
        }
    }

    /// Reset both epochs to zero. Called on full engine reset (paste, resize, etc.).
    pub fn reset(&mut self) {
        self.prediction_epoch = 0;
        self.confirmed_epoch = 0;
    }

    /// Kill predictions from the given epoch forward.
    ///
    /// Called when a misprediction is detected on a tentative cell. Advances
    /// the prediction epoch so all predictions tagged >= the kill point are
    /// considered stale. Returns the new prediction epoch.
    pub fn kill_epoch(&mut self, from_epoch: u64) -> u64 {
        if from_epoch <= self.prediction_epoch {
            // Move prediction epoch past all killed predictions
            self.prediction_epoch = self.prediction_epoch.saturating_add(1);
            // Align confirmed to match so we don't leave a gap that can never close
            self.confirmed_epoch = self.prediction_epoch;
        }
        self.prediction_epoch
    }
}

impl Default for EpochTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tracker_has_zero_epochs() {
        let tracker = EpochTracker::new();
        assert_eq!(tracker.prediction_epoch(), 0);
        assert_eq!(tracker.confirmed_epoch(), 0);
    }

    #[test]
    fn become_tentative_increments_prediction_epoch() {
        let mut tracker = EpochTracker::new();
        tracker.become_tentative();
        assert_eq!(tracker.prediction_epoch(), 1);
        assert_eq!(tracker.confirmed_epoch(), 0);
    }

    #[test]
    fn is_confirmed_at_epoch_zero_always_true() {
        let tracker = EpochTracker::new();
        // A prediction tagged with epoch 0 is visible immediately since confirmed == 0
        assert!(tracker.is_confirmed(0));
    }

    #[test]
    fn prediction_after_tentative_not_confirmed() {
        let mut tracker = EpochTracker::new();
        tracker.become_tentative();
        // A prediction tagged with the new epoch (1) is not yet confirmed
        assert!(!tracker.is_confirmed(1));
        assert!(tracker.has_tentative_predictions());
    }

    #[test]
    fn advance_confirmed_makes_tentative_visible() {
        let mut tracker = EpochTracker::new();
        tracker.become_tentative();
        tracker.advance_confirmed();
        assert!(tracker.is_confirmed(1));
        assert!(!tracker.has_tentative_predictions());
    }

    #[test]
    fn confirmed_cannot_exceed_prediction_epoch() {
        let mut tracker = EpochTracker::new();
        // Advance confirmed without any predictions — should stay at 0
        tracker.advance_confirmed();
        assert_eq!(tracker.confirmed_epoch(), 0);
    }

    #[test]
    fn reset_clears_both_epochs() {
        let mut tracker = EpochTracker::new();
        tracker.become_tentative();
        tracker.become_tentative();
        tracker.advance_confirmed();
        tracker.reset();
        assert_eq!(tracker.prediction_epoch(), 0);
        assert_eq!(tracker.confirmed_epoch(), 0);
    }

    #[test]
    fn kill_epoch_advances_past_stale_predictions() {
        let mut tracker = EpochTracker::new();
        tracker.become_tentative(); // prediction_epoch = 1
        tracker.become_tentative(); // prediction_epoch = 2
        let new_epoch = tracker.kill_epoch(1);
        // After kill, prediction epoch is higher and confirmed catches up
        assert!(new_epoch > 2);
        assert_eq!(tracker.confirmed_epoch(), tracker.prediction_epoch());
    }

    #[test]
    fn saturating_add_prevents_overflow() {
        let mut tracker = EpochTracker {
            prediction_epoch: u64::MAX,
            confirmed_epoch: u64::MAX,
        };
        // Should not panic — saturating_add stops at MAX
        tracker.become_tentative();
        assert_eq!(tracker.prediction_epoch(), u64::MAX);
    }
}
