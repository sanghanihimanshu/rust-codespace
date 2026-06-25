use anyhow::{Context, Result};
use crossbeam_channel::Sender;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

use scap::{
    capturer::{Capturer, Options, Resolution},
    frame::{Frame, FrameType},
    Target,
};

use super::RawVideoFrame;

/// Returns human-readable display names for the capture target picker.
pub fn list_displays() -> Vec<String> {
    scap::get_all_targets()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .filter_map(|(i, t)| match t {
            Target::Display(d) => Some(format!("Display {} (id {})", i, d.id)),
            _ => None,
        })
        .collect()
}

/// Blocking capture loop — run on a dedicated thread.
/// Sends BGRA frames into `tx` until `stop_rx` fires.
pub fn run_capture(
    display_index: usize,
    fps: u32,
    tx: Sender<RawVideoFrame>,
    stop_rx: crossbeam_channel::Receiver<()>,
) -> Result<()> {
    let target = scap::get_all_targets()
        .unwrap_or_default()
        .into_iter()
        .filter(|t| matches!(t, Target::Display(_)))
        .nth(display_index);

    let options = Options {
        fps,
        target,
        show_cursor: true,
        show_highlight: false,
        excluded_targets: None,
        output_type: FrameType::BGRAFrame,
        output_resolution: Resolution::_1080p,
        crop_area: None,
    };

    let mut capturer = Capturer::build(options).context("Failed to create screen capturer")?;

    info!("Screen capture started at {} fps", fps);
    capturer.start_capture();

    loop {
        if stop_rx.try_recv().is_ok() {
            break;
        }

        match capturer.get_next_frame() {
            Ok(frame) => {
                let pts_us = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros() as u64;

                let raw = match frame {
                    Frame::BGRA(f) => RawVideoFrame {
                        data: f.data,
                        width: f.width as u32,
                        height: f.height as u32,
                        pts_us,
                    },
                    Frame::BGR0(f) => {
                        let mut data = f.data;
                        for chunk in data.chunks_exact_mut(4) {
                            chunk[3] = 255;
                        }
                        RawVideoFrame {
                            data,
                            width: f.width as u32,
                            height: f.height as u32,
                            pts_us,
                        }
                    }
                    _ => {
                        warn!("Unsupported frame type, skipping");
                        continue;
                    }
                };

                if tx.send(raw).is_err() {
                    break;
                }
            }
            Err(e) => {
                warn!("Frame receive error: {e}");
                break;
            }
        }
    }

    capturer.stop_capture();
    info!("Screen capture stopped");
    Ok(())
}
