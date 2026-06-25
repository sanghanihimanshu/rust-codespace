pub mod audio;
pub mod screen;

/// Raw video frame arriving from the capture backend.
#[derive(Clone)]
pub struct RawVideoFrame {
    /// BGRA pixel data, row-major, width × height × 4 bytes.
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Monotonic capture timestamp in microseconds.
    pub pts_us: u64,
}

/// Raw audio chunk arriving from CPAL.
#[derive(Clone)]
pub struct RawAudioChunk {
    /// Interleaved f32 PCM samples.
    pub samples: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
    pub pts_us: u64,
}
