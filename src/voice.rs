//! Voice input module for Shift+Enter push-to-talk.
//!
//! Provides audio capture via cpal (CoreAudio on macOS) and in-process
//! transcription via whisper-rs (whisper.cpp bindings).

use anyhow::{Context, Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SampleRate, StreamConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Configuration for the voice input subsystem.
#[derive(Debug, Clone)]
pub struct VoiceConfig {
    /// Whether voice input is enabled at all.
    pub enabled: bool,
    /// Path to the whisper GGML model file.
    pub whisper_model: String,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            whisper_model: resolve_model_path(),
        }
    }
}

/// Resolve the whisper model path.
///
/// Checks (in order):
/// 1. Inside the .app bundle: `<exe>/../Resources/ggml-base.en.bin`
/// 2. The build-time path baked in by build.rs (works for `cargo run`)
///
/// Panics if no model is found — the app cannot function without it.
pub fn resolve_model_path() -> String {
    let model_name = "ggml-base.en.bin";

    // Check .app bundle first (distributed builds).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(macos_dir) = exe.parent() {
            let resources = macos_dir.join("../Resources").join(model_name);
            if resources.exists() {
                return resources.to_string_lossy().to_string();
            }
        }
    }

    // Fall back to the path baked in at compile time by build.rs.
    let build_path = env!("WHISPER_MODEL_PATH");
    if std::path::Path::new(build_path).exists() {
        return build_path.to_string();
    }

    panic!(
        "Whisper model not found. Expected in .app bundle Resources or at build path: {}",
        build_path
    );
}

/// Load or create a WhisperContext from the given model path.
pub fn load_whisper_context(model_path: &str) -> Result<WhisperContext> {
    WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
        .map_err(|e| anyhow!("Failed to load whisper model '{}': {:?}", model_path, e))
}

/// Live recording state — held while Shift+Enter is depressed.
pub struct RecordingState {
    /// Accumulated PCM samples (f32, interleaved if multi-channel).
    pub buffer: Arc<Mutex<Vec<f32>>>,
    /// Set to false to signal the stream callback to stop appending.
    pub active: Arc<AtomicBool>,
    /// The live cpal stream. Kept alive by this struct; dropped on stop.
    pub _stream: cpal::Stream,
    /// Sample rate the audio was captured at.
    pub sample_rate: u32,
    /// Number of channels captured.
    pub channels: u16,
}

/// Describes the current voice UI state, used by dashboard for visual feedback.
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceUiState {
    Idle,
    Recording,
    Transcribing,
    /// Transcription completed; carries the text for display.
    Done(String),
    Error(String),
}

// ---------------------------------------------------------------------------
// Audio capture
// ---------------------------------------------------------------------------

/// Start a cpal input stream, accumulating f32 samples in a shared buffer.
///
/// Returns a `RecordingState` that keeps the stream alive until dropped.
pub fn start_recording() -> Result<RecordingState> {
    let host = cpal::default_host();

    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("No audio input device found. Check microphone permissions."))?;

    let supported_config = device
        .default_input_config()
        .context("Failed to get default input config")?;

    let sample_rate = supported_config.sample_rate().0;
    let channels = supported_config.channels();

    let stream_config = StreamConfig {
        channels,
        sample_rate: SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let active = Arc::new(AtomicBool::new(true));

    let buffer_clone = Arc::clone(&buffer);
    let active_clone = Arc::clone(&active);

    let stream = match supported_config.sample_format() {
        SampleFormat::F32 => build_f32_stream(&device, &stream_config, buffer_clone, active_clone),
        SampleFormat::I16 => build_i16_stream(&device, &stream_config, buffer_clone, active_clone),
        SampleFormat::U16 => build_u16_stream(&device, &stream_config, buffer_clone, active_clone),
        fmt => Err(anyhow!("Unsupported sample format: {:?}", fmt)),
    }?;

    stream.play().context("Failed to start audio stream")?;

    Ok(RecordingState {
        buffer,
        active,
        _stream: stream,
        sample_rate,
        channels,
    })
}

fn build_f32_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    active: Arc<AtomicBool>,
) -> Result<cpal::Stream> {
    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _| {
            if active.load(Ordering::Relaxed) {
                if let Ok(mut buf) = buffer.lock() {
                    buf.extend_from_slice(data);
                }
            }
        },
        |err| eprintln!("Audio stream error: {}", err),
        None,
    )?;
    Ok(stream)
}

fn build_i16_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    active: Arc<AtomicBool>,
) -> Result<cpal::Stream> {
    let stream = device.build_input_stream(
        config,
        move |data: &[i16], _| {
            if active.load(Ordering::Relaxed) {
                if let Ok(mut buf) = buffer.lock() {
                    buf.extend(data.iter().map(|&s| s as f32 / i16::MAX as f32));
                }
            }
        },
        |err| eprintln!("Audio stream error: {}", err),
        None,
    )?;
    Ok(stream)
}

fn build_u16_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    active: Arc<AtomicBool>,
) -> Result<cpal::Stream> {
    let stream = device.build_input_stream(
        config,
        move |data: &[u16], _| {
            if active.load(Ordering::Relaxed) {
                if let Ok(mut buf) = buffer.lock() {
                    buf.extend(
                        data.iter()
                            .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0),
                    );
                }
            }
        },
        |err| eprintln!("Audio stream error: {}", err),
        None,
    )?;
    Ok(stream)
}

// ---------------------------------------------------------------------------
// Audio preparation (pure functions)
// ---------------------------------------------------------------------------

/// Prepare raw captured audio for whisper: mix to mono, resample to 16kHz.
///
/// Returns f32 samples ready to pass directly to whisper-rs `full()`.
pub fn prepare_audio_for_whisper(
    samples: &[f32],
    source_sample_rate: u32,
    channels: u16,
) -> Vec<f32> {
    // Mix down to mono by averaging channels.
    let mono: Vec<f32> = if channels == 1 {
        samples.to_vec()
    } else {
        samples
            .chunks_exact(channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    // Resample to 16 kHz using linear interpolation if needed.
    let target_rate: u32 = 16_000;
    if source_sample_rate == target_rate {
        mono
    } else {
        resample_linear(&mono, source_sample_rate, target_rate)
    }
}

/// Linear interpolation resampler: converts `input` at `from_rate` Hz to `to_rate` Hz.
fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if input.is_empty() {
        return Vec::new();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = ((input.len() as f64) / ratio).ceil() as usize;
    (0..output_len)
        .map(|i| {
            let src_pos = i as f64 * ratio;
            let src_idx = src_pos as usize;
            let frac = (src_pos - src_idx as f64) as f32;
            let a = input[src_idx.min(input.len() - 1)];
            let b = input[(src_idx + 1).min(input.len() - 1)];
            a + (b - a) * frac
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Transcription (in-process via whisper-rs)
// ---------------------------------------------------------------------------

/// Transcription result returned from the whisper backend.
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
}

/// Transcribe f32 PCM audio (16kHz mono) using a pre-loaded WhisperContext.
///
/// This is a blocking, CPU-bound operation — call from `spawn_blocking`.
pub fn transcribe_local(
    ctx: &WhisperContext,
    audio: &[f32],
) -> Result<TranscriptionResult> {
    let mut state = ctx.create_state().map_err(|e| anyhow!("whisper state: {:?}", e))?;

    let mut params = FullParams::new(SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 });
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_language(Some("en"));
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);

    state
        .full(params, audio)
        .map_err(|e| anyhow!("whisper inference failed: {:?}", e))?;

    let num_segments = state.full_n_segments();
    let mut text = String::new();
    for i in 0..num_segments {
        if let Some(segment) = state.get_segment(i) {
            if let Ok(s) = segment.to_str_lossy() {
                text.push_str(&s);
            }
        }
    }

    Ok(TranscriptionResult {
        text: text.trim().to_string(),
    })
}

// ---------------------------------------------------------------------------
// Stop and collect helper
// ---------------------------------------------------------------------------

/// Signal the recording to stop and extract the accumulated buffer.
pub fn stop_and_collect(state: &RecordingState) -> Vec<f32> {
    state.active.store(false, Ordering::Relaxed);
    std::thread::sleep(std::time::Duration::from_millis(20));
    state.buffer.lock().unwrap().clone()
}
