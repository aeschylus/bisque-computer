//! Voice input state machine.
//!
//! Hierarchy:
//! ```text
//! Disabled ←→ Enabled (superstate)
//!                 ├── Idle
//!                 ├── Recording  [entry: start capture, exit: stop capture + transcribe]
//!                 ├── Transcribing
//!                 ├── Done { text }
//!                 └── Error { msg }
//! ```

use anyhow::Result;
use statig::prelude::*;
use std::sync::Arc;
use tracing::{error, info, warn};
use whisper_rs::WhisperContext;

use crate::voice::{RecordingState, VoiceConfig, VoiceUiState};

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Events dispatched to the voice state machine.
#[derive(Debug, Clone)]
pub enum VoiceEvent {
    /// User pressed 'V' to toggle voice on.
    Enable,
    /// User pressed 'V' to toggle voice off.
    Disable,
    /// Shift+Enter pressed (start push-to-talk).
    ShiftEnterPressed,
    /// Shift+Enter or Shift released (stop recording).
    StopRecording,
    /// Async transcription task completed successfully.
    TranscriptionComplete(String),
    /// Async transcription task failed.
    TranscriptionFailed(String),
    /// Reset the Done/Error state back to Idle (e.g. after a delay).
    Reset,
}

// ---------------------------------------------------------------------------
// Shared storage
// ---------------------------------------------------------------------------

/// Shared storage for the voice state machine.
///
/// Holds the resources that must persist across transitions:
/// - `config`: immutable voice settings (model path, etc.)
/// - `recording`: live capture handle; `Some` only in `Recording` state
/// - `whisper_ctx`: cached WhisperContext to avoid reloading the model
/// - `transcription_tx/rx`: channel bridging the async task back to the event loop
pub struct VoiceMachine {
    pub config: VoiceConfig,
    pub recording: Option<RecordingState>,
    pub whisper_ctx: Option<Arc<WhisperContext>>,
    pub transcription_tx:
        std::sync::mpsc::SyncSender<Result<crate::voice::TranscriptionResult, String>>,
    pub transcription_rx:
        std::sync::mpsc::Receiver<Result<crate::voice::TranscriptionResult, String>>,
    pub rt_handle: tokio::runtime::Handle,
    pub outbound: crate::ws_client::OutboundSender,
}

impl VoiceMachine {
    pub fn new(
        config: VoiceConfig,
        rt_handle: tokio::runtime::Handle,
        outbound: crate::ws_client::OutboundSender,
    ) -> Self {
        let (transcription_tx, transcription_rx) =
            std::sync::mpsc::sync_channel::<Result<crate::voice::TranscriptionResult, String>>(4);

        // Eagerly load the whisper model if voice is enabled.
        let whisper_ctx = if config.enabled {
            match crate::voice::load_whisper_context(&config.whisper_model) {
                Ok(ctx) => {
                    info!(target: "voice", model = %config.whisper_model, "Whisper model loaded");
                    Some(Arc::new(ctx))
                }
                Err(e) => {
                    warn!(target: "voice", "Failed to load whisper model: {}. Transcription will fail.", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            config,
            recording: None,
            whisper_ctx,
            transcription_tx,
            transcription_rx,
            rt_handle,
            outbound,
        }
    }

    /// Return a `VoiceUiState` snapshot for rendering.
    pub fn ui_state(state: &State) -> VoiceUiState {
        match state {
            State::Disabled {} => VoiceUiState::Idle,
            State::Idle {} => VoiceUiState::Idle,
            State::Recording {} => VoiceUiState::Recording,
            State::Transcribing {} => VoiceUiState::Transcribing,
            State::Done { text } => VoiceUiState::Done(text.clone()),
            State::Error { msg } => VoiceUiState::Error(msg.clone()),
        }
    }

    /// Poll the transcription channel. Returns an event if a result arrived.
    ///
    /// Should be called once per frame from the render loop.
    pub fn poll_transcription(&self) -> Option<VoiceEvent> {
        match self.transcription_rx.try_recv() {
            Ok(Ok(result)) => {
                let text = result.text.clone();
                info!(target: "voice", "Transcribed: {:?}", text);

                if !text.is_empty() {
                    let msg = crate::protocol::VoiceInputMessage::new(text.clone());
                    self.outbound.broadcast(msg.to_json());
                }

                Some(VoiceEvent::TranscriptionComplete(text))
            }
            Ok(Err(e)) => {
                error!(target: "voice", "Transcription failed: {}", e);
                Some(VoiceEvent::TranscriptionFailed(e))
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// State machine implementation
// ---------------------------------------------------------------------------

#[state_machine(
    initial = "State::idle()",
    state(derive(Debug, Clone, PartialEq))
)]
impl VoiceMachine {
    // ------------------------------------------------------------------
    // Superstate: Enabled (parent of Idle, Recording, Transcribing, Done, Error)
    // ------------------------------------------------------------------

    #[superstate]
    fn enabled(&mut self, event: &VoiceEvent) -> Outcome<State> {
        match event {
            VoiceEvent::Disable => Transition(State::disabled()),
            _ => Super,
        }
    }

    // ------------------------------------------------------------------
    // Leaf states
    // ------------------------------------------------------------------

    /// Voice input is disabled. Ignores all voice events except Enable.
    #[state]
    fn disabled(&mut self, event: &VoiceEvent) -> Outcome<State> {
        match event {
            VoiceEvent::Enable => Transition(State::idle()),
            _ => Handled,
        }
    }

    /// Waiting for Shift+Enter. Child of `enabled`.
    #[state(superstate = "enabled")]
    fn idle(&mut self, event: &VoiceEvent) -> Outcome<State> {
        match event {
            VoiceEvent::ShiftEnterPressed => Transition(State::recording()),
            VoiceEvent::Reset => Handled,
            _ => Super,
        }
    }

    /// Audio is being captured. Entry starts cpal stream; exit stops it.
    #[state(
        superstate = "enabled",
        entry_action = "enter_recording",
        exit_action = "exit_recording"
    )]
    fn recording(&mut self, event: &VoiceEvent) -> Outcome<State> {
        match event {
            VoiceEvent::StopRecording => Transition(State::transcribing()),
            _ => Super,
        }
    }

    /// Waiting for the async whisper task to return.
    #[state(superstate = "enabled", entry_action = "enter_transcribing")]
    fn transcribing(&mut self, event: &VoiceEvent) -> Outcome<State> {
        match event {
            VoiceEvent::TranscriptionComplete(text) => {
                Transition(State::done(text.clone()))
            }
            VoiceEvent::TranscriptionFailed(msg) => {
                Transition(State::error(msg.clone()))
            }
            _ => Super,
        }
    }

    /// Transcription succeeded; `text` carries the result.
    #[state(superstate = "enabled")]
    fn done(&mut self, event: &VoiceEvent, text: &String) -> Outcome<State> {
        let _ = text; // used only for rendering via ui_state()
        match event {
            VoiceEvent::Reset => Transition(State::idle()),
            VoiceEvent::ShiftEnterPressed => Transition(State::recording()),
            _ => Super,
        }
    }

    /// Transcription failed; `msg` carries the error description.
    #[state(superstate = "enabled")]
    fn error(&mut self, event: &VoiceEvent, msg: &String) -> Outcome<State> {
        let _ = msg;
        match event {
            VoiceEvent::Reset => Transition(State::idle()),
            VoiceEvent::ShiftEnterPressed => Transition(State::recording()),
            _ => Super,
        }
    }

    // ------------------------------------------------------------------
    // Entry / exit actions
    // ------------------------------------------------------------------

    /// Start cpal audio capture. Called on entry to `Recording`.
    #[action]
    fn enter_recording(&mut self) {
        if !self.config.enabled {
            return;
        }
        match crate::voice::start_recording() {
            Ok(rec) => {
                self.recording = Some(rec);
                info!(target: "voice", "Recording started");
            }
            Err(e) => {
                error!(target: "voice", "Failed to start recording: {}", e);
            }
        }
    }

    /// Stop cpal audio capture and spawn the transcription task.
    /// Called on exit from `Recording`.
    #[action]
    fn exit_recording(&mut self) {
        let rec = match self.recording.take() {
            Some(r) => r,
            None => return,
        };

        let whisper_ctx = match &self.whisper_ctx {
            Some(ctx) => Arc::clone(ctx),
            None => {
                let _ = self.transcription_tx.send(Err(
                    "Whisper model not loaded. Cannot transcribe.".to_string(),
                ));
                return;
            }
        };

        let samples = crate::voice::stop_and_collect(&rec);
        let sample_rate = rec.sample_rate;
        let channels = rec.channels;
        let tx = self.transcription_tx.clone();

        // Run whisper inference on a blocking thread to avoid blocking the event loop.
        self.rt_handle.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                let audio =
                    crate::voice::prepare_audio_for_whisper(&samples, sample_rate, channels);
                crate::voice::transcribe_local(&whisper_ctx, &audio)
                    .map_err(|e| e.to_string())
            })
            .await;

            let send_result = match result {
                Ok(inner) => inner,
                Err(e) => Err(format!("Transcription task panicked: {}", e)),
            };
            let _ = tx.send(send_result);
        });

        info!(target: "voice", "Recording stopped, transcribing...");
    }

    /// Log entry to Transcribing state.
    #[action]
    fn enter_transcribing(&mut self) {
        info!(target: "voice", "Transcribing...");
    }
}
