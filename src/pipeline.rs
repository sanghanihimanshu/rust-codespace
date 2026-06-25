//! Pipeline: capture → encode → SRT-mux, running on background threads.
//!
//! The pipeline is started by `Pipeline::start()` and shut down by dropping
//! the returned `PipelineHandle` or calling `PipelineHandle::stop()`.

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::Mutex;
use tracing::{error, info, warn};

use crate::app_state::{AppState, SharedState, StreamStats, StreamStatus};
use crate::capture::{RawAudioChunk, RawVideoFrame};
use crate::capture::{audio::AudioCapturer, screen};
use crate::config::StreamConfig;
use crate::encode::{EncodedPacket, audio::AudioEncoder, video::VideoEncoder};
use crate::stream::{
    discovery::MdnsAnnouncer,
    srt_output::{run_mux_loop, SrtMuxer},
};

const CHANNEL_CAP: usize = 32;

/// Handle returned to the caller.  Drop (or call `stop()`) to shut down.
pub struct PipelineHandle {
    stop_tx: Option<Sender<()>>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

impl PipelineHandle {
    pub fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for PipelineHandle {
    fn drop(&mut self) {
        self.stop();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

pub fn start(config: StreamConfig, state: SharedState) -> Result<PipelineHandle> {
    // Channels: capture → encode
    let (video_raw_tx, video_raw_rx) = bounded::<RawVideoFrame>(CHANNEL_CAP);
    let (audio_raw_tx, audio_raw_rx) = bounded::<RawAudioChunk>(CHANNEL_CAP);

    // Channels: encode → mux
    let (video_enc_tx, video_enc_rx) = bounded::<EncodedPacket>(CHANNEL_CAP);
    let (audio_enc_tx, audio_enc_rx) = bounded::<EncodedPacket>(CHANNEL_CAP);

    // Stop channels (broadcast via clone)
    let (stop_tx, stop_rx) = bounded::<()>(1);
    let stop_rx_screen = stop_rx.clone();
    let stop_rx_mux = stop_rx.clone();

    let cfg = config.clone();
    let state_clone = state.clone();

    // ── Thread 1: Screen capture ──────────────────────────────────────────────
    let cfg_screen = cfg.clone();
    let screen_thread = std::thread::Builder::new()
        .name("screen-capture".into())
        .spawn(move || {
            if let Err(e) = screen::run_capture(
                cfg_screen.display_index,
                cfg_screen.fps,
                video_raw_tx,
                stop_rx_screen,
            ) {
                error!("Screen capture error: {e:#}");
            }
        })?;

    // ── Thread 2: Audio capture (mic) ─────────────────────────────────────────
    let cfg_audio = cfg.clone();
    let audio_thread = std::thread::Builder::new()
        .name("audio-capture".into())
        .spawn(move || {
            let device = cfg_audio.mic_device.as_deref();
            match AudioCapturer::start(device, audio_raw_tx) {
                Ok(_capturer) => {
                    // Capturer keeps the stream alive; block here until stop
                    stop_rx.recv().ok();
                }
                Err(e) => {
                    error!("Audio capture error: {e:#}");
                }
            }
        })?;

    // ── Thread 3: Video encode ────────────────────────────────────────────────
    let cfg_venc = cfg.clone();
    let state_venc = state.clone();
    let video_enc_thread = std::thread::Builder::new()
        .name("video-encode".into())
        .spawn(move || {
            // We don't know resolution yet; get it from the first frame.
            let mut encoder: Option<VideoEncoder> = None;
            let mut last_fps_ts = Instant::now();
            let mut frame_count = 0u32;

            loop {
                let frame = match video_raw_rx.recv() {
                    Ok(f) => f,
                    Err(_) => break,
                };

                // Lazy init encoder on first frame (we now know src dims).
                if encoder.is_none() {
                    let (w, h) = cfg_venc.resolve_dims(frame.width, frame.height);
                    match VideoEncoder::new(&cfg_venc.video_codec, w, h, cfg_venc.fps, cfg_venc.video_kbps) {
                        Ok(enc) => encoder = Some(enc),
                        Err(e) => {
                            error!("Cannot create video encoder: {e:#}");
                            break;
                        }
                    }
                }

                let enc = encoder.as_mut().unwrap();
                let t0 = Instant::now();
                match enc.encode(&frame) {
                    Ok(pkts) => {
                        let encode_ms = t0.elapsed().as_secs_f32() * 1000.0;
                        frame_count += 1;

                        for pkt in pkts {
                            if video_enc_tx.send(pkt).is_err() {
                                return;
                            }
                        }

                        // Update stats roughly every second
                        if last_fps_ts.elapsed().as_secs_f32() >= 1.0 {
                            let fps = frame_count as f32 / last_fps_ts.elapsed().as_secs_f32();
                            let mut st = state_venc.lock();
                            st.stats.actual_fps = fps;
                            st.stats.encode_ms = encode_ms;
                            last_fps_ts = Instant::now();
                            frame_count = 0;
                        }
                    }
                    Err(e) => warn!("Video encode error: {e:#}"),
                }
            }

            // Flush encoder
            if let Some(enc) = encoder.as_mut() {
                if let Ok(pkts) = enc.flush() {
                    for pkt in pkts {
                        let _ = video_enc_tx.send(pkt);
                    }
                }
            }
        })?;

    // ── Thread 4: Audio encode ────────────────────────────────────────────────
    let cfg_aenc = cfg.clone();
    let audio_enc_thread = std::thread::Builder::new()
        .name("audio-encode".into())
        .spawn(move || {
            // Lazy init once we know the source format.
            let mut encoder: Option<AudioEncoder> = None;

            loop {
                let chunk = match audio_raw_rx.recv() {
                    Ok(c) => c,
                    Err(_) => break,
                };

                if encoder.is_none() {
                    match AudioEncoder::new(chunk.sample_rate, chunk.channels, cfg_aenc.audio_kbps) {
                        Ok(enc) => encoder = Some(enc),
                        Err(e) => {
                            error!("Cannot create audio encoder: {e:#}");
                            break;
                        }
                    }
                }

                let enc = encoder.as_mut().unwrap();
                match enc.encode(&chunk) {
                    Ok(pkts) => {
                        for pkt in pkts {
                            if audio_enc_tx.send(pkt).is_err() {
                                return;
                            }
                        }
                    }
                    Err(e) => warn!("Audio encode error: {e:#}"),
                }
            }

            if let Some(enc) = encoder.as_mut() {
                if let Ok(pkts) = enc.flush() {
                    for pkt in pkts {
                        let _ = audio_enc_tx.send(pkt);
                    }
                }
            }
        })?;

    // ── Thread 5: SRT mux + mDNS ─────────────────────────────────────────────
    let cfg_mux = cfg.clone();
    let state_mux = state.clone();
    let mux_thread = std::thread::Builder::new()
        .name("srt-mux".into())
        .spawn(move || {
            // Advertise via mDNS
            let mdns = MdnsAnnouncer::start(
                &cfg_mux.service_name,
                cfg_mux.port,
                &cfg_mux.passphrase,
            )
            .map_err(|e| warn!("mDNS error (non-fatal): {e:#}"))
            .ok();

            // Build the SRT muxer (blocks until OBS connects)
            info!("Waiting for SRT client (OBS) to connect…");
            let res_rate = cfg_mux.fps;
            let muxer = SrtMuxer::connect(
                &cfg_mux,
                1920, 1080, // placeholder; real dims come from first frame
                res_rate,
                cfg_mux.video_kbps,
                48000,
                cfg_mux.audio_kbps,
            );

            match muxer {
                Ok(mut mx) => {
                    {
                        let mut st = state_mux.lock();
                        st.status = StreamStatus::Streaming;
                    }
                    info!("SRT client connected — streaming");

                    if let Err(e) = run_mux_loop(&mut mx, video_enc_rx, audio_enc_rx, stop_rx_mux) {
                        warn!("Mux loop ended: {e:#}");
                    }

                    let _ = mx.close();
                }
                Err(e) => {
                    error!("SRT muxer failed: {e:#}");
                    let mut st = state_mux.lock();
                    st.status = StreamStatus::Error(e.to_string());
                }
            }

            if let Some(m) = mdns {
                m.stop();
            }

            let mut st = state_mux.lock();
            if matches!(st.status, StreamStatus::Streaming | StreamStatus::Starting) {
                st.status = StreamStatus::Idle;
            }
        })?;

    {
        let mut st = state_clone.lock();
        st.status = StreamStatus::Starting;
    }

    Ok(PipelineHandle {
        stop_tx: Some(stop_tx),
        threads: vec![screen_thread, audio_thread, video_enc_thread, audio_enc_thread, mux_thread],
    })
}
