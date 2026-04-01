#[repr(C)]
#[derive(Debug)]
pub struct DriverArgs {
    pub pin_func: [u32; 4],
    pub pads: [u32; 2],
    pub resets: u32
}

pub const PAD_DISABLE: u32 = 0;
pub const PAD_NORMAL: u32 = 1;
pub const PAD_ANALOG: u32 = 2;
pub const PAD_PULL_UP: u32 = 3;

impl DriverArgs {
    pub fn new() -> Self {
        let mut res = Self {
            pin_func: [u32::MAX; 4],
            pads: [0; 2],
            resets: 0
        };
        // Make kernel UART1 pad normal and set func sel as UART
        res.pads[0] |= PAD_NORMAL << 8;
        res.pin_func[0] &= !0xf0000;
        res.pin_func[0] |= 2 << 16;
        res
    }

    pub fn serialise(&self) -> Vec<u8> {
        println!("Have args {:#x?}", self);
        let mut res = Vec::new();
        res.extend_from_slice(&self.pin_func[0].to_le_bytes());
        res.extend_from_slice(&self.pin_func[1].to_le_bytes());
        res.extend_from_slice(&self.pin_func[2].to_le_bytes());
        res.extend_from_slice(&self.pin_func[3].to_le_bytes());
        res.extend_from_slice(&self.pads[0].to_le_bytes());
        res.extend_from_slice(&self.pads[1].to_le_bytes());
        res.extend_from_slice(&self.resets.to_le_bytes());
        assert!(res.len() == std::mem::size_of::<DriverArgs>());
        res
    }
}
