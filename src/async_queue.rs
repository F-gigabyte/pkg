pub struct AsyncQueue {
    pub buffer: u32,
    pub buffer_len: u32,
    pub message_len: u32,
}

impl AsyncQueue {
    pub fn serialise(&self) -> Vec<u8> {
        let mut res = Vec::new();
        res.extend_from_slice(&self.buffer.to_le_bytes());
        res.extend_from_slice(&self.buffer_len.to_le_bytes());
        res.extend_from_slice(&self.message_len.to_le_bytes());
        res.extend_from_slice(&0u32.to_le_bytes());
        res.extend_from_slice(&0u32.to_le_bytes());
        res.extend_from_slice(&0u32.to_le_bytes());
        res.extend_from_slice(&0u32.to_le_bytes());
        res
    }
}
