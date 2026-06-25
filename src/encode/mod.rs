pub mod audio;
pub mod video;

/// Encoded packet ready for the muxer.
pub struct EncodedPacket {
    pub data: Vec<u8>,
    pub pts_us: i64,
    pub dts_us: i64,
    pub is_key: bool,
    pub stream_index: usize, // 0 = video, 1 = audio
}
