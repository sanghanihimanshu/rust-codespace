use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;
use ffmpeg::software::scaling;
use tracing::{debug, info};

use crate::capture::RawVideoFrame;
use crate::config::VideoCodec;
use super::EncodedPacket;

pub struct VideoEncoder {
    /// Opened H.264/H.265 encoder (ffmpeg_next::encoder::Video = encoder::video::Encoder)
    encoder: ffmpeg::encoder::Video,
    scaler: scaling::Context,
    yuv_frame: ffmpeg::frame::Video,
    time_base_us: i64,
    frame_count: u64,
}

impl VideoEncoder {
    pub fn new(
        codec: &VideoCodec,
        width: u32,
        height: u32,
        fps: u32,
        kbps: u32,
    ) -> Result<Self> {
        ffmpeg::init().context("FFmpeg init failed")?;

        let codec_obj = ffmpeg::encoder::find_by_name(codec.ffmpeg_name())
            .with_context(|| format!("{} encoder not found", codec.ffmpeg_name()))?;

        let mut ctx = ffmpeg::codec::context::Context::new_with_codec(codec_obj);
        let mut builder = ctx.encoder().video()?;

        builder.set_width(width);
        builder.set_height(height);
        builder.set_format(ffmpeg::format::Pixel::YUV420P);
        builder.set_time_base(ffmpeg::Rational::new(1, fps as i32));
        builder.set_bit_rate((kbps as usize) * 1_000);
        builder.set_gop(fps * 2);
        builder.set_max_b_frames(0);

        let mut dict = ffmpeg::Dictionary::new();
        dict.set("tune", "zerolatency");
        dict.set("preset", "ultrafast");

        // open_with returns encoder::Video (= encoder::video::Encoder)
        let encoder = builder.open_with(dict).context("Failed to open video encoder")?;

        let scaler = scaling::Context::get(
            ffmpeg::format::Pixel::BGRA,
            width, height,
            ffmpeg::format::Pixel::YUV420P,
            width, height,
            scaling::Flags::BILINEAR,
        )
        .context("Failed to create pixel-format scaler")?;

        let yuv_frame = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::YUV420P, width, height);

        info!(
            "Video encoder ready: {} {}×{} @ {} fps, {} kbps",
            codec.display_name(), width, height, fps, kbps
        );

        Ok(Self {
            encoder,
            scaler,
            yuv_frame,
            time_base_us: 1_000_000 / fps as i64,
            frame_count: 0,
        })
    }

    pub fn encode(&mut self, frame: &RawVideoFrame) -> Result<Vec<EncodedPacket>> {
        // Fill a temporary BGRA source frame
        let mut bgra = ffmpeg::frame::Video::new(
            ffmpeg::format::Pixel::BGRA,
            frame.width,
            frame.height,
        );
        {
            let stride = bgra.stride(0);
            let row_bytes = frame.width as usize * 4;
            let data = bgra.data_mut(0);
            for row in 0..frame.height as usize {
                let src = &frame.data[row * row_bytes..(row + 1) * row_bytes];
                let dst = &mut data[row * stride..(row * stride + row_bytes)];
                dst.copy_from_slice(src);
            }
        }

        self.scaler.run(&bgra, &mut self.yuv_frame).context("Pixel-format conversion failed")?;

        self.yuv_frame.set_pts(Some(self.frame_count as i64));
        self.frame_count += 1;

        self.encoder.send_frame(&self.yuv_frame).context("send_frame failed")?;
        self.drain_packets()
    }

    pub fn flush(&mut self) -> Result<Vec<EncodedPacket>> {
        self.encoder.send_eof()?;
        self.drain_packets()
    }

    fn drain_packets(&mut self) -> Result<Vec<EncodedPacket>> {
        let mut out = Vec::new();
        let mut pkt = ffmpeg::Packet::empty();
        loop {
            match self.encoder.receive_packet(&mut pkt) {
                Ok(()) => {
                    let pts_us = pkt.pts().unwrap_or(0) * self.time_base_us;
                    let dts_us = pkt.dts().unwrap_or(pts_us) * self.time_base_us;
                    debug!("Video packet: {} bytes, key={}", pkt.size(), pkt.is_key());
                    out.push(EncodedPacket {
                        data: pkt.data().unwrap_or(&[]).to_vec(),
                        pts_us,
                        dts_us,
                        is_key: pkt.is_key(),
                        stream_index: 0,
                    });
                }
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => break,
                Err(ffmpeg::Error::Eof) => break,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(out)
    }
}
