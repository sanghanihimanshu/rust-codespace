use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VideoCodec {
    H264,
    H265,
}

impl VideoCodec {
    pub fn ffmpeg_name(&self) -> &'static str {
        match self {
            Self::H264 => "libx264",
            Self::H265 => "libx265",
        }
    }
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::H264 => "H.264 (AVC)",
            Self::H265 => "H.265 (HEVC)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Resolution {
    Native,
    R1080p,
    R720p,
    R480p,
}

impl Resolution {
    pub fn dims(&self) -> Option<(u32, u32)> {
        match self {
            Self::Native => None,
            Self::R1080p => Some((1920, 1080)),
            Self::R720p => Some((1280, 720)),
            Self::R480p => Some((854, 480)),
        }
    }
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Native => "Source (native)",
            Self::R1080p => "1080p",
            Self::R720p => "720p",
            Self::R480p => "480p",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    /// SRT listen port
    pub port: u16,
    /// AES-256 passphrase for SRT encryption (empty = no encryption)
    pub passphrase: String,
    /// H.264 or H.265
    pub video_codec: VideoCodec,
    /// Video bitrate in kbps
    pub video_kbps: u32,
    /// Output resolution (None = native)
    pub resolution: Resolution,
    /// Frames per second
    pub fps: u32,
    /// Audio bitrate in kbps
    pub audio_kbps: u32,
    /// Which capture display index (0 = primary)
    pub display_index: usize,
    /// Capture system audio loopback (requires monitor source on Linux)
    pub capture_system_audio: bool,
    /// Mic device name; None = default
    pub mic_device: Option<String>,
    /// Network interface to bind (None = bind all / 0.0.0.0)
    pub bind_interface: Option<String>,
    /// mDNS service name advertised on LAN
    pub service_name: String,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            port: 4200,
            passphrase: "screenstream_secret".into(),
            video_codec: VideoCodec::H264,
            video_kbps: 8000,
            resolution: Resolution::R1080p,
            fps: 60,
            audio_kbps: 192,
            display_index: 0,
            capture_system_audio: false,
            mic_device: None,
            bind_interface: None,
            service_name: "ScreenStream".into(),
        }
    }
}

impl StreamConfig {
    /// Resolve (width, height) for the given source dimensions.
    pub fn resolve_dims(&self, src_w: u32, src_h: u32) -> (u32, u32) {
        self.resolution.dims().unwrap_or((src_w, src_h))
    }

    /// SRT URL that OBS uses to connect.
    pub fn obs_srt_url(&self, host: &str) -> String {
        if self.passphrase.is_empty() {
            format!("srt://{}:{}", host, self.port)
        } else {
            format!(
                "srt://{}:{}?passphrase={}&pbkeylen=32",
                host, self.port, self.passphrase
            )
        }
    }

    /// Local SRT listener URL for FFmpeg output.
    pub fn srt_listen_url(&self) -> String {
        let bind = self
            .bind_interface
            .as_deref()
            .unwrap_or("0.0.0.0");
        if self.passphrase.is_empty() {
            format!("srt://{}:{}?mode=listener", bind, self.port)
        } else {
            format!(
                "srt://{}:{}?mode=listener&passphrase={}&pbkeylen=32",
                bind, self.port, self.passphrase
            )
        }
    }
}
