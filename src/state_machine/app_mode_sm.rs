//! App-mode state machine.
//!
//! Models the top-level display mode:
//! ```text
//! Setup → Dashboard
//! ```

use statig::prelude::*;
use tracing::info;

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Events dispatched to the app-mode state machine.
#[derive(Debug, Clone)]
pub enum AppModeEvent {
    /// User submitted a valid WebSocket URL in setup mode.
    UrlSubmitted(String),
}

// ---------------------------------------------------------------------------
// Shared storage
// ---------------------------------------------------------------------------

/// Shared storage for the app-mode state machine.
///
/// Holds everything that belongs to setup mode (URL input buffer) and the
/// spawned WebSocket client handles.
pub struct AppModeMachine {
    pub setup_input: String,
    pub instances: crate::ws_client::SharedInstances,
    pub outbound: crate::ws_client::OutboundSender,
}

impl AppModeMachine {
    pub fn new(
        instances: crate::ws_client::SharedInstances,
        outbound: crate::ws_client::OutboundSender,
    ) -> Self {
        Self {
            setup_input: String::new(),
            instances,
            outbound,
        }
    }
}

// ---------------------------------------------------------------------------
// State machine implementation
// ---------------------------------------------------------------------------

#[state_machine(
    initial = "State::setup()",
    state(derive(Debug, Clone, PartialEq))
)]
impl AppModeMachine {
    /// Waiting for the user to enter a server URL.
    #[state]
    fn setup(&mut self, event: &AppModeEvent) -> Outcome<State> {
        match event {
            AppModeEvent::UrlSubmitted(url) => {
                info!(target: "setup", "URL submitted: {}", url);
                // Spawn WS clients for the new URL and transition to Dashboard.
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create runtime for setup");
                let (instances, outbound) =
                    crate::ws_client::spawn_clients(&rt, vec![url.clone()]);
                // Leak the runtime so spawned tasks keep running.
                std::mem::forget(rt);
                self.instances = instances;
                self.outbound = outbound;
                Transition(State::dashboard())
            }
        }
    }

    /// Normal operation: connected (or connecting) to a Lobster server.
    #[state]
    fn dashboard(&mut self, event: &AppModeEvent) -> Outcome<State> {
        let _ = event;
        Handled
    }
}
