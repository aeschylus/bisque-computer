//! Voice input module for Shift+Enter push-to-talk.
//!
//! Provides audio capture via cpal (CoreAudio on macOS), WAV encoding via hound,
//! and transcription via whisper.cpp's HTTP server sidecar.
//!
//! Design: pure functions for WAV encoding and transcription; side-effectful
//! operations (stream start/stop, HTTP call) are isolated at the boundaries.

use anyhow::{Context, Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SampleRate, StreamConfig};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Configuration for the voice input subsystem.
#[derive(Debug, Clone)]
pub struct VoiceConfig {
    /// Whether voice input is enabled at all.
    pub enabled: bool,
    /// Path to the whisper-cli binary.
    pub whisper_cli: String,
    /// Path to the whisper model file.
    pub whisper_model: String,
    /// Hostname or IP of the whisper.cpp HTTP server (default: "localhost").
    /// Set to the Lobster server host so the Mac client can reach it remotely.
    pub whisper_host: String,
    /// Port for the whisper.cpp HTTP server sidecar.
    pub whisper_port: u16,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            whisper_cli: "/home/admin/lobster-workspace/whisper.cpp/build/bin/whisper-cli"
                .to_string(),
            whisper_model: "/home/admin/lobster-workspace/whisper.cpp/models/ggml-small.bin"
                .to_string(),
            whisper_host: "localhost".to_string(),
            whisper_port: 8178,
        }
    }
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
/// The caller stops recording by dropping the returned value (or calling
/// `stop_and_encode`).
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

    // Build the input stream based on the device's native sample format,
    // always converting to f32 for uniform downstream processing.
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
// WAV encoding (pure function — no side effects)
// ---------------------------------------------------------------------------

/// Encode accumulated f32 PCM samples into an in-memory WAV byte vector.
///
/// Whisper expects 16 kHz mono 16-bit PCM. If the source has a different
/// sample rate or multiple channels, we downsample and mix down here.
pub fn encode_to_wav(
    samples: &[f32],
    source_sample_rate: u32,
    channels: u16,
) -> Result<Vec<u8>> {
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
    let resampled: Vec<f32> = if source_sample_rate == target_rate {
        mono
    } else {
        resample_linear(&mono, source_sample_rate, target_rate)
    };

    // Convert f32 [-1.0, 1.0] → i16
    let pcm_i16: Vec<i16> = resampled
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect();

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: target_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut cursor, spec)
        .context("Failed to create WAV writer")?;

    for sample in &pcm_i16 {
        writer.write_sample(*sample).context("Failed to write WAV sample")?;
    }
    writer.finalize().context("Failed to finalize WAV")?;

    Ok(cursor.into_inner())
}

/// Linear interpolation resampler: converts `input` at `from_rate` Hz to `to_rate` Hz.
///
/// Pure function — no side effects.
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
// Transcription
// ---------------------------------------------------------------------------

/// Transcription result returned from the whisper backend.
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
}

/// Call the whisper.cpp HTTP server to transcribe a WAV byte blob.
///
/// Sends a multipart POST to `http://{host}:{port}/inference`.
/// Returns the trimmed transcription text on success.
pub async fn transcribe_via_server(
    wav_bytes: Vec<u8>,
    host: &str,
    port: u16,
) -> Result<TranscriptionResult> {
    let url = format!("http://{}:{}/inference", host, port);

    let part = reqwest::multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")?;

    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("response-format", "json");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let response = client
        .post(&url)
        .multipart(form)
        .send()
        .await
        .context("Failed to reach whisper.cpp server. Is it running?")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Whisper server returned {}: {}", status, body));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse whisper response JSON")?;

    // whisper.cpp server returns {"text": "...", ...}
    let text = json["text"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    Ok(TranscriptionResult { text })
}

/// Transcribe WAV bytes using whisper-cli as a subprocess.
///
/// Writes WAV to a temp file, invokes whisper-cli, parses stdout.
/// This is the fallback path when the HTTP server is not running.
pub async fn transcribe_via_cli(
    wav_bytes: Vec<u8>,
    whisper_cli: &str,
    whisper_model: &str,
) -> Result<TranscriptionResult> {
    use std::io::Write;

    // Write WAV to a temp file
    let tmp_path = std::env::temp_dir().join("bisque_voice_input.wav");
    {
        let mut f = std::fs::File::create(&tmp_path)
            .context("Failed to create temp WAV file")?;
        f.write_all(&wav_bytes)
            .context("Failed to write temp WAV file")?;
    }

    let output = tokio::process::Command::new(whisper_cli)
        .args([
            "--model",
            whisper_model,
            "--file",
            tmp_path.to_str().unwrap(),
            "--no-timestamps",
            "--output-txt",
            "-",
        ])
        .output()
        .await
        .context("Failed to run whisper-cli")?;

    // Clean up temp file (best-effort)
    let _ = std::fs::remove_file(&tmp_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("whisper-cli failed: {}", stderr));
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    // whisper-cli outputs lines like "[00:00:00.000 --> 00:00:05.000]  text here"
    // With --no-timestamps, it may still include brackets on some builds.
    // Strip any leading timestamp brackets if present.
    let text = raw
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.starts_with('[') {
                // Strip "[HH:MM:SS.mmm --> HH:MM:SS.mmm]  " prefix
                trimmed.find(']').map(|i| trimmed[i + 1..].trim().to_string())
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    Ok(TranscriptionResult { text })
}

/// Attempt transcription: try HTTP server first, fall back to CLI subprocess.
pub async fn transcribe(wav_bytes: Vec<u8>, config: &VoiceConfig) -> Result<TranscriptionResult> {
    match transcribe_via_server(wav_bytes.clone(), &config.whisper_host, config.whisper_port).await {
        Ok(result) => Ok(result),
        Err(server_err) => {
            eprintln!(
                "whisper HTTP server unavailable ({}), falling back to CLI",
                server_err
            );
            transcribe_via_cli(wav_bytes, &config.whisper_cli, &config.whisper_model).await
        }
    }
}

// ---------------------------------------------------------------------------
// Stop and encode helper
// ---------------------------------------------------------------------------

/// Signal the recording to stop and extract the accumulated buffer.
///
/// Marks the stream as inactive and collects the PCM samples. The stream
/// itself is kept alive by `RecordingState`; drop it after calling this.
pub fn stop_and_collect(state: &RecordingState) -> Vec<f32> {
    state.active.store(false, Ordering::Relaxed);
    // Give the stream callback one more tick to flush
    std::thread::sleep(std::time::Duration::from_millis(20));
    state.buffer.lock().unwrap().clone()
}
