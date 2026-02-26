#[derive(Debug)]
pub struct Region {
    pub phys_addr: u32,
    pub virt_addr: u32,
    pub len: u32
}

impl Region {
    pub const ENABLE_SHIFT: usize = 0;
    pub const PERM_SHIFT: usize = 1;
    pub const DEVICE_SHIFT: usize = 4;
    pub const ZERO_SHIFT: usize = 5;

    pub const ENABLE_MASK: u32 = 1 << Self::ENABLE_SHIFT;
    pub const DEVICE_MASK: u32 = 1 << Self::DEVICE_SHIFT;
    pub const ZERO_MASK: u32 = 1 << Self::ZERO_SHIFT;
    pub const PERM_MASK: u32 = 0x7 << Self::PERM_SHIFT;
    pub const ADDR_MASK: u32 = 0xffffff00;

    pub const fn default() -> Self {
        Self { phys_addr: 0, virt_addr: 0, len: 0 }
    }

    pub fn serialise(&self) -> Vec<u8> {
        let mut res = Vec::new();
        res.extend_from_slice(&self.phys_addr.to_ne_bytes());
        res.extend_from_slice(&self.virt_addr.to_ne_bytes());
        res.extend_from_slice(&self.len.to_ne_bytes());
        res
    }

    pub const fn get_region_size() -> usize {
        std::mem::size_of::<u32>() * 3
    }
}

