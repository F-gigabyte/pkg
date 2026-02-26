struct SyncEndpoint {
    pub queue_addr: u32
}

impl SyncEndpoint {
    pub fn serialise(&self) -> Vec<u8> {
        let mut res = Vec::new();
        res.extend_from_slice(&self.queue_addr.to_le_bytes());
        res
    }
}

struct AsyncEndpoint {
    pub queue_addr: u32
}

impl AsyncEndpoint {
    pub fn serialise(&self) -> Vec<u8> {
        let mut res = Vec::new();
        res.extend_from_slice(&self.queue_addr.to_le_bytes());
        res
    }
}

