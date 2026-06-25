use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Sender;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

use super::RawAudioChunk;

/// List available audio input device names.
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut names = vec!["Default".into()];
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            if let Ok(name) = d.name() {
                names.push(name);
            }
        }
    }
    names
}

/// Opens the chosen input device and streams audio chunks into `tx`
/// until the returned `StopHandle` is dropped.
pub struct AudioCapturer {
    _stream: cpal::Stream,
}

impl AudioCapturer {
    pub fn start(
        device_name: Option<&str>,
        tx: Sender<RawAudioChunk>,
    ) -> Result<Self> {
        let host = cpal::default_host();

        let device = if let Some(name) = device_name {
            host.input_devices()
                .context("Cannot enumerate audio devices")?
                .find(|d| d.name().ok().as_deref() == Some(name))
                .context("Audio device not found")?
        } else {
            host.default_input_device()
                .context("No default audio input device")?
        };

        info!("Audio capture device: {}", device.name().unwrap_or_default());

        let supported = device
            .default_input_config()
            .context("No default input config")?;

        let sample_rate = supported.sample_rate().0;
        let channels = supported.channels();

        let config = cpal::StreamConfig {
            channels,
            sample_rate: supported.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        let err_tx = tx.clone();
        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], _info| {
                    let pts_us = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_micros() as u64;
                    let chunk = RawAudioChunk {
                        samples: data.to_vec(),
                        channels,
                        sample_rate,
                        pts_us,
                    };
                    let _ = tx.send(chunk);
                },
                move |e| {
                    warn!("Audio stream error: {e}");
                    let _ = err_tx; // keep tx alive
                },
                None,
            )
            .context("Failed to build audio input stream")?;

        stream.play().context("Failed to start audio stream")?;
        info!("Audio capture started ({} Hz, {} ch)", sample_rate, channels);

        Ok(Self { _stream: stream })
    }
}

/// On Linux, the PulseAudio/PipeWire monitor source captures system audio.
/// Returns the monitor source device name if found, else None.
#[cfg(target_os = "linux")]
pub fn find_system_audio_device() -> Option<String> {
    let host = cpal::default_host();
    host.input_devices().ok()?.find_map(|d| {
        let name = d.name().ok()?;
        // PulseAudio monitor sources end in ".monitor"
        if name.ends_with(".monitor") {
            Some(name)
        } else {
            None
        }
    })
}

#[cfg(not(target_os = "linux"))]
pub fn find_system_audio_device() -> Option<String> {
    None
}
