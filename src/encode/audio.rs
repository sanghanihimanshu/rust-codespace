use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;
use ffmpeg::software::resampling;
use tracing::{debug, info};

use crate::capture::RawAudioChunk;
use super::EncodedPacket;

pub struct AudioEncoder {
    /// Opened AAC encoder (ffmpeg_next::encoder::Audio = encoder::audio::Encoder)
    encoder: ffmpeg::encoder::Audio,
    resampler: Option<resampling::Context>,
    out_sample_rate: u32,
    frame_size: usize,
    buf: Vec<f32>,
    pts: i64,
    out_channels: u16,
}

impl AudioEncoder {
    pub fn new(src_sample_rate: u32, src_channels: u16, kbps: u32) -> Result<Self> {
        ffmpeg::init().context("FFmpeg init failed")?;

        let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::AAC)
            .context("AAC encoder not found")?;

        let mut ctx = ffmpeg::codec::context::Context::new_with_codec(codec);
        let mut builder = ctx.encoder().audio()?;

        let out_rate = if src_sample_rate <= 44_100 { 44_100u32 } else { 48_000u32 };
        let out_layout = ffmpeg::channel_layout::ChannelLayout::STEREO;

        builder.set_rate(out_rate as i32);
        builder.set_channel_layout(out_layout);
        builder.set_format(ffmpeg::format::Sample::F32(
            ffmpeg::format::sample::Type::Packed,
        ));
        builder.set_bit_rate((kbps as usize) * 1_000);
        builder.set_time_base(ffmpeg::Rational::new(1, out_rate as i32));

        // open() returns encoder::Audio (= encoder::audio::Encoder)
        let encoder = builder.open().context("Failed to open AAC encoder")?;
        let frame_size = encoder.frame_size() as usize;

        // Build a resampler if the source format doesn't match the encoder
        let needs_resample = src_sample_rate != out_rate || src_channels != 2;
        let resampler = if needs_resample {
            let src_layout = if src_channels == 1 {
                ffmpeg::channel_layout::ChannelLayout::MONO
            } else {
                ffmpeg::channel_layout::ChannelLayout::STEREO
            };
            Some(
                resampling::Context::get(
                    ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                    src_layout,
                    src_sample_rate,
                    ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                    out_layout,
                    out_rate,
                )
                .context("Failed to create audio resampler")?,
            )
        } else {
            None
        };

        info!(
            "Audio encoder ready: AAC {}→{} Hz, {} kbps, frame_size={}",
            src_sample_rate, out_rate, kbps, frame_size
        );

        Ok(Self {
            encoder,
            resampler,
            out_sample_rate: out_rate,
            frame_size,
            buf: Vec::new(),
            pts: 0,
            out_channels: 2,
        })
    }

    pub fn encode(&mut self, chunk: &RawAudioChunk) -> Result<Vec<EncodedPacket>> {
        let stereo: Vec<f32> = if let Some(rsp) = &mut self.resampler {
            let samples_per_ch = chunk.samples.len() / chunk.channels as usize;
            let src_layout = if chunk.channels == 1 {
                ffmpeg::channel_layout::ChannelLayout::MONO
            } else {
                ffmpeg::channel_layout::ChannelLayout::STEREO
            };

            let mut src_frame = ffmpeg::frame::Audio::new(
                ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                samples_per_ch,
                src_layout,
            );
            {
                let data = src_frame.data_mut(0);
                let bytes = f32_to_bytes(&chunk.samples);
                data[..bytes.len()].copy_from_slice(bytes);
            }

            let mut dst = ffmpeg::frame::Audio::empty();
            rsp.run(&src_frame, &mut dst)?;
            bytes_to_f32(dst.data(0)).to_vec()
        } else {
            chunk.samples.clone()
        };

        self.buf.extend_from_slice(&stereo);
        self.drain_buf()
    }

    pub fn flush(&mut self) -> Result<Vec<EncodedPacket>> {
        self.encoder.send_eof()?;
        self.drain_encoder()
    }

    fn drain_buf(&mut self) -> Result<Vec<EncodedPacket>> {
        let per_frame = self.frame_size * self.out_channels as usize;
        let mut out = Vec::new();
        while self.buf.len() >= per_frame {
            let samples: Vec<f32> = self.buf.drain(..per_frame).collect();
            out.extend(self.encode_frame(&samples)?);
        }
        Ok(out)
    }

    fn encode_frame(&mut self, samples: &[f32]) -> Result<Vec<EncodedPacket>> {
        let num_samples = samples.len() / self.out_channels as usize;

        let mut frame = ffmpeg::frame::Audio::new(
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            num_samples,
            ffmpeg::channel_layout::ChannelLayout::STEREO,
        );
        frame.set_pts(Some(self.pts));
        self.pts += num_samples as i64;

        {
            let data = frame.data_mut(0);
            let bytes = f32_to_bytes(samples);
            data[..bytes.len()].copy_from_slice(bytes);
        }

        self.encoder.send_frame(&frame)?;
        self.drain_encoder()
    }

    fn drain_encoder(&mut self) -> Result<Vec<EncodedPacket>> {
        let mut out = Vec::new();
        let mut pkt = ffmpeg::Packet::empty();
        loop {
            match self.encoder.receive_packet(&mut pkt) {
                Ok(()) => {
                    let pts_us =
                        pkt.pts().unwrap_or(0) * 1_000_000 / self.out_sample_rate as i64;
                    debug!("Audio packet: {} bytes", pkt.size());
                    out.push(EncodedPacket {
                        data: pkt.data().unwrap_or(&[]).to_vec(),
                        pts_us,
                        dts_us: pts_us,
                        is_key: false,
                        stream_index: 1,
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

// SAFETY: f32 has no padding; we read the raw bytes directly.
fn f32_to_bytes(s: &[f32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, s.len() * 4) }
}

fn bytes_to_f32(b: &[u8]) -> &[f32] {
    unsafe { std::slice::from_raw_parts(b.as_ptr() as *const f32, b.len() / 4) }
}
