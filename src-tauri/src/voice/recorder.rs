//! cpal-based mic capture to a WAV file inside `app_cache_dir/voice/`.
//!
//! Capture rules:
//! - We pick the default input device and request the configured sample
//!   rate as mono. If the device cannot satisfy the request, we accept its
//!   default config and resample / downmix into the target on write.
//! - The stream is owned inside a worker thread because `cpal::Stream` is
//!   `!Send` on most backends. A blocking channel hands raw samples to the
//!   thread, which writes them through `hound::WavWriter`.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use tauri::{AppHandle, Manager};

/// Active recording handle: spec, output path, control channel.
struct ActiveRecording {
    path: PathBuf,
    target_rate: u32,
    stop_tx: mpsc::Sender<RecorderCmd>,
    worker: JoinHandle<Result<PathBuf, String>>,
}

#[derive(Default)]
pub struct VoiceRecorderState {
    inner: Mutex<HashMap<String, ActiveRecording>>,
}

impl VoiceRecorderState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

enum RecorderCmd {
    Stop,
    Cancel,
}

pub fn cache_root(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("app cache dir unavailable: {e}"))?;
    let dir = base.join("voice");
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Start a recording. Returns the assigned turn id.
pub fn start(
    app: &AppHandle,
    state: &VoiceRecorderState,
    target_rate: u32,
) -> Result<String, String> {
    let turn_id = uuid::Uuid::new_v4().to_string();
    let dir = cache_root(app)?;
    let path = dir.join(format!("{turn_id}.wav"));

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "Kein Default-Audio-Eingang gefunden.".to_string())?;
    let device_name = device.name().unwrap_or_else(|_| "unknown".into());
    let supported = device
        .default_input_config()
        .map_err(|e| format!("default_input_config({device_name}): {e}"))?;

    let input_rate = supported.sample_rate();
    let channels = supported.channels() as usize;
    let target_path = path.clone();

    let (sample_tx, sample_rx) = mpsc::channel::<Vec<f32>>();
    let (cmd_tx, cmd_rx) = mpsc::channel::<RecorderCmd>();

    let stream_sample_format = supported.sample_format();
    let stream_config: cpal::StreamConfig = supported.into();

    let worker_path = target_path.clone();
    let worker = thread::spawn(move || -> Result<PathBuf, String> {
        let stream = build_stream(
            &device,
            &stream_config,
            stream_sample_format,
            sample_tx.clone(),
        )?;
        stream.play().map_err(|e| format!("stream.play: {e}"))?;

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: target_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&worker_path, spec)
            .map_err(|e| format!("wav create {}: {e}", worker_path.display()))?;

        let mut resampler = LinearResampler::new(input_rate, target_rate);
        let mut cancelled = false;
        loop {
            match cmd_rx.try_recv() {
                Ok(RecorderCmd::Stop) => break,
                Ok(RecorderCmd::Cancel) => {
                    cancelled = true;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
            match sample_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                Ok(buf) => {
                    let mono = downmix_to_mono(&buf, channels);
                    let resampled = resampler.process(&mono);
                    for sample in resampled {
                        let clamped = sample.clamp(-1.0, 1.0);
                        let s16 = (clamped * i16::MAX as f32) as i16;
                        writer
                            .write_sample(s16)
                            .map_err(|e| format!("wav write: {e}"))?;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        drop(stream);
        // Drain any remaining samples in the channel.
        while let Ok(buf) = sample_rx.try_recv() {
            let mono = downmix_to_mono(&buf, channels);
            let resampled = resampler.process(&mono);
            for sample in resampled {
                let clamped = sample.clamp(-1.0, 1.0);
                let s16 = (clamped * i16::MAX as f32) as i16;
                writer
                    .write_sample(s16)
                    .map_err(|e| format!("wav write: {e}"))?;
            }
        }
        writer
            .finalize()
            .map_err(|e| format!("wav finalize: {e}"))?;
        if cancelled {
            let _ = std::fs::remove_file(&worker_path);
            return Err("cancelled".into());
        }
        Ok(worker_path)
    });

    let mut map = state
        .inner
        .lock()
        .map_err(|_| "recorder state poisoned".to_string())?;
    map.insert(
        turn_id.clone(),
        ActiveRecording {
            path,
            target_rate,
            stop_tx: cmd_tx,
            worker,
        },
    );
    Ok(turn_id)
}

pub fn stop(state: &VoiceRecorderState, turn_id: &str) -> Result<PathBuf, String> {
    let recording = take_recording(state, turn_id)?;
    let _ = recording.stop_tx.send(RecorderCmd::Stop);
    recording
        .worker
        .join()
        .map_err(|_| "recorder worker panicked".to_string())?
}

pub fn cancel(state: &VoiceRecorderState, turn_id: &str) -> Result<(), String> {
    let recording = take_recording(state, turn_id)?;
    let _ = recording.stop_tx.send(RecorderCmd::Cancel);
    // We ignore the worker's outcome here — Err("cancelled") is the expected path.
    let _ = recording.worker.join();
    let _ = std::fs::remove_file(&recording.path);
    let _ = recording.target_rate;
    Ok(())
}

fn take_recording(state: &VoiceRecorderState, turn_id: &str) -> Result<ActiveRecording, String> {
    let mut map = state
        .inner
        .lock()
        .map_err(|_| "recorder state poisoned".to_string())?;
    map.remove(turn_id)
        .ok_or_else(|| format!("Keine aktive Aufnahme für {turn_id}."))
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    tx: mpsc::Sender<Vec<f32>>,
) -> Result<cpal::Stream, String> {
    let err_fn = |e| eprintln!("voice recorder stream error: {e}");
    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _| {
                let _ = tx.send(data.to_vec());
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _| {
                let buf = data
                    .iter()
                    .map(|s| *s as f32 / i16::MAX as f32)
                    .collect::<Vec<f32>>();
                let _ = tx.send(buf);
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            config,
            move |data: &[u16], _| {
                let buf = data
                    .iter()
                    .map(|s| (*s as f32 - 32_768.0) / 32_768.0)
                    .collect::<Vec<f32>>();
                let _ = tx.send(buf);
            },
            err_fn,
            None,
        ),
        other => return Err(format!("Unsupported sample format: {other:?}")),
    };
    stream.map_err(|e| format!("build_input_stream: {e}"))
}

fn downmix_to_mono(buf: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return buf.to_vec();
    }
    let mut out = Vec::with_capacity(buf.len() / channels);
    for frame in buf.chunks_exact(channels) {
        let sum: f32 = frame.iter().sum();
        out.push(sum / channels as f32);
    }
    out
}

struct LinearResampler {
    input_rate: u32,
    output_rate: u32,
    ratio: f64,
    position: f64,
    last_sample: f32,
}

impl LinearResampler {
    fn new(input_rate: u32, output_rate: u32) -> Self {
        Self {
            input_rate,
            output_rate,
            ratio: input_rate as f64 / output_rate as f64,
            position: 0.0,
            last_sample: 0.0,
        }
    }

    fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        if self.input_rate == self.output_rate {
            return samples.to_vec();
        }
        if samples.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        // Working buffer that includes the carry-over last sample so we can
        // interpolate across chunk boundaries.
        let mut buf = Vec::with_capacity(samples.len() + 1);
        buf.push(self.last_sample);
        buf.extend_from_slice(samples);
        let max_index = (buf.len() - 1) as f64;
        while self.position <= max_index {
            let idx = self.position.floor() as usize;
            let frac = (self.position - idx as f64) as f32;
            let a = buf[idx];
            let b = if idx + 1 < buf.len() { buf[idx + 1] } else { a };
            out.push(a + (b - a) * frac);
            self.position += self.ratio;
        }
        // Re-anchor: keep the position relative to the new last sample.
        self.position -= max_index;
        if self.position < 0.0 {
            self.position = 0.0;
        }
        self.last_sample = *samples.last().unwrap_or(&self.last_sample);
        out
    }
}
