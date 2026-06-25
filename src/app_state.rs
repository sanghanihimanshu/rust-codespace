use std::sync::Arc;
use parking_lot::Mutex;
use crate::config::StreamConfig;

#[derive(Debug, Clone, PartialEq)]
pub enum StreamStatus {
    Idle,
    Starting,
    Streaming,
    Stopping,
    Error(String),
}

impl StreamStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Idle => "Idle",
            Self::Starting => "Starting…",
            Self::Streaming => "Streaming",
            Self::Stopping => "Stopping…",
            Self::Error(_) => "Error",
        }
    }
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Starting | Self::Streaming)
    }
}

/// Real-time stats updated from the pipeline thread.
#[derive(Debug, Clone, Default)]
pub struct StreamStats {
    pub actual_fps: f32,
    pub video_kbps: f32,
    pub audio_kbps: f32,
    pub frames_encoded: u64,
    pub encode_ms: f32,
    pub connected_clients: u32,
}

/// Shared application state — used by both the GPUI view and the pipeline.
/// Wrapped in Arc<Mutex<>> so the pipeline thread can write to it and the
/// GPUI main thread can read it inside `render()`.
#[derive(Debug)]
pub struct AppState {
    pub status: StreamStatus,
    pub config: StreamConfig,
    pub stats: StreamStats,
    pub local_ips: Vec<String>,
    pub available_interfaces: Vec<String>,
    pub available_displays: Vec<String>,
    pub available_audio_devices: Vec<String>,
    /// Signalled by the pipeline to stop streaming.
    pub stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl AppState {
    pub fn new() -> Self {
        let local_ips = collect_local_ips();
        let available_interfaces = collect_interfaces();
        Self {
            status: StreamStatus::Idle,
            config: StreamConfig::default(),
            stats: StreamStats::default(),
            local_ips,
            available_interfaces,
            available_displays: vec!["Display 0 (Primary)".into()],
            available_audio_devices: vec!["Default".into()],
            stop_tx: None,
        }
    }

    pub fn primary_obs_url(&self) -> String {
        let host = self.local_ips.first().map(|s| s.as_str()).unwrap_or("127.0.0.1");
        self.config.obs_srt_url(host)
    }
}

/// Thread-safe wrapper passed to the pipeline task.
pub type SharedState = Arc<Mutex<AppState>>;

fn collect_local_ips() -> Vec<String> {
    let mut ips = Vec::new();
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        for iface in ifaces {
            let ip = iface.addr.ip();
            if ip.is_loopback() {
                continue;
            }
            ips.push(ip.to_string());
        }
    }
    if ips.is_empty() {
        ips.push("127.0.0.1".into());
    }
    ips
}

fn collect_interfaces() -> Vec<String> {
    let mut names = vec!["All interfaces (0.0.0.0)".into()];
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        let mut seen = std::collections::HashSet::new();
        for iface in ifaces {
            if iface.addr.ip().is_loopback() {
                continue;
            }
            if seen.insert(iface.name.clone()) {
                names.push(iface.name);
            }
        }
    }
    names
}
