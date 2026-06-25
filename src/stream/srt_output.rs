use anyhow::{Context, Result};
use crossbeam_channel::Receiver;
use ffmpeg_next as ffmpeg;
use tracing::info;

use crate::config::StreamConfig;
use crate::encode::EncodedPacket;

// Microsecond time base — our encoder timestamps are in µs
const US_TB: ffmpeg::Rational = ffmpeg::Rational(1, 1_000_000);

/// MPEG-TS muxer over FFmpeg's built-in SRT transport.
///
/// The SRT listener waits for OBS (or any SRT client) to connect.
/// AES-256 encryption is applied by FFmpeg via `passphrase` + `pbkeylen=32`.
pub struct SrtMuxer {
    octx: ffmpeg::format::context::Output,
    video_idx: usize,
    audio_idx: usize,
    video_tb: ffmpeg::Rational,
    audio_tb: ffmpeg::Rational,
}

impl SrtMuxer {
    /// Open the SRT listener.  Blocks until the first client connects
    /// (or errors out, e.g. if SRT is not compiled into FFmpeg).
    pub fn connect(
        config: &StreamConfig,
        video_width: u32,
        video_height: u32,
        video_fps: u32,
        video_kbps: u32,
        audio_sample_rate: u32,
        audio_kbps: u32,
    ) -> Result<Self> {
        ffmpeg::init()?;

        let srt_url = config.srt_listen_url();
        info!("Opening SRT listener: {}", srt_url);

        let mut octx = ffmpeg::format::output_as(&srt_url, "mpegts")
            .with_context(|| format!("Cannot open SRT output at {srt_url}"))?;

        // ── Video stream ─────────────────────────────────────────────────────
        let video_codec = ffmpeg::encoder::find_by_name(config.video_codec.ffmpeg_name())
            .context("Video codec not found in FFmpeg")?;

        let mut vst = octx.add_stream(video_codec)?;
        let video_tb = ffmpeg::Rational::new(1, video_fps as i32);
        vst.set_time_base(video_tb);

        // Populate stream codec parameters from a temporary encoder context
        {
            let mut vctx = ffmpeg::codec::context::Context::new_with_codec(video_codec);
            let mut enc = vctx.encoder().video()?;
            enc.set_width(video_width);
            enc.set_height(video_height);
            enc.set_format(ffmpeg::format::Pixel::YUV420P);
            enc.set_time_base(video_tb);
            enc.set_bit_rate((video_kbps as usize) * 1_000);

            let mut dict = ffmpeg::Dictionary::new();
            dict.set("tune", "zerolatency");
            dict.set("preset", "ultrafast");
            let opened = enc.open_with(dict)?;
            vst.set_parameters(&opened);
        }
        let video_idx = vst.index();

        // ── Audio stream ─────────────────────────────────────────────────────
        let audio_codec =
            ffmpeg::encoder::find(ffmpeg::codec::Id::AAC).context("AAC codec not found")?;

        let mut ast = octx.add_stream(audio_codec)?;
        let audio_tb = ffmpeg::Rational::new(1, audio_sample_rate as i32);
        ast.set_time_base(audio_tb);

        {
            let mut actx = ffmpeg::codec::context::Context::new_with_codec(audio_codec);
            let mut enc = actx.encoder().audio()?;
            enc.set_rate(audio_sample_rate as i32);
            enc.set_channel_layout(ffmpeg::channel_layout::ChannelLayout::STEREO);
            enc.set_format(ffmpeg::format::Sample::F32(
                ffmpeg::format::sample::Type::Packed,
            ));
            enc.set_bit_rate((audio_kbps as usize) * 1_000);
            enc.set_time_base(audio_tb);
            let opened = enc.open()?;
            ast.set_parameters(&opened);
        }
        let audio_idx = ast.index();

        // Write MPEG-TS PAT/PMT — blocks until OBS connects for SRT
        octx.write_header()
            .context("SRT write_header failed (no client connected yet?)")?;

        info!(
            "SRT stream open — OBS URL: {}",
            config.obs_srt_url("YOUR_LAN_IP")
        );

        Ok(Self {
            octx,
            video_idx,
            audio_idx,
            video_tb,
            audio_tb,
        })
    }

    /// Write one encoded packet (already in µs timestamps) to the mux.
    pub fn write(&mut self, pkt: &EncodedPacket) -> Result<()> {
        let (stream_idx, tb) = if pkt.stream_index == 0 {
            (self.video_idx, self.video_tb)
        } else {
            (self.audio_idx, self.audio_tb)
        };

        let mut packet = ffmpeg::Packet::copy(pkt.data.as_slice());
        packet.set_stream(stream_idx);
        packet.set_pts(Some(pkt.pts_us));
        packet.set_dts(Some(pkt.dts_us));
        if pkt.is_key {
            packet.set_flags(ffmpeg::codec::packet::Flags::KEY);
        }

        // Rescale timestamps from µs → stream time base
        packet.rescale_ts(US_TB, tb);

        // write_interleaved is a method on Packet, not on Output
        packet
            .write_interleaved(&mut self.octx)
            .context("SRT write_interleaved failed")
    }

    /// Flush and close.
    pub fn close(mut self) -> Result<()> {
        self.octx.write_trailer()?;
        Ok(())
    }
}

/// Blocking mux loop — drains video_rx and audio_rx until stop or disconnect.
pub fn run_mux_loop(
    muxer: &mut SrtMuxer,
    video_rx: Receiver<EncodedPacket>,
    audio_rx: Receiver<EncodedPacket>,
    stop_rx: Receiver<()>,
) -> Result<()> {
    use crossbeam_channel::select;

    loop {
        select! {
            recv(video_rx) -> pkt => match pkt {
                Ok(p)  => muxer.write(&p)?,
                Err(_) => break,
            },
            recv(audio_rx) -> pkt => match pkt {
                Ok(p)  => muxer.write(&p)?,
                Err(_) => break,
            },
            recv(stop_rx) -> _ => break,
        }
    }
    Ok(())
}
