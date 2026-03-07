use crate::section_attr::SectionAttr;

#[derive(Debug)]
pub struct Region {
    // name is just there for reference but doesn't end up in the final binary
    pub name: Option<String>,
    pub phys_addr: u32,
    pub virt_addr: u32,
    pub len: u32,
    pub actual_len: u32,
    pub codes: u32,
}

impl Region {
    // len mask
    pub const ENABLE_SHIFT: usize = 0;
    pub const VIRTUAL_SHIFT: usize = 1;
    pub const PHYSICAL_SHIFT: usize = 2;
    pub const DEVICE_SHIFT: usize = 3;
    pub const PERM_SHIFT: usize = 4;
    pub const ZERO_SHIFT: usize = 6;
    pub const LEN_SHIFT: usize = 16;

    pub const ENABLE_MASK: u32 = 1 << Self::ENABLE_SHIFT;
    pub const VIRTUAL_MASK: u32 = 1 << Self::VIRTUAL_SHIFT;
    pub const PHYSICAL_MASK: u32 = 1 << Self::PHYSICAL_SHIFT;
    pub const DEVICE_MASK: u32 = 1 << Self::DEVICE_SHIFT;
    pub const PERM_MASK: u32 = 0x3 << Self::PERM_SHIFT;
    pub const ZERO_MASK: u32 = 1 << Self::ZERO_SHIFT;
    pub const LEN_MASK: u32 = 0xffff << Self::LEN_SHIFT;

    pub const fn default() -> Self {
        Self { 
            name: None,
            phys_addr: 0, 
            virt_addr: 0, 
            len: 0,
            actual_len: 0,
            codes: 0,
        }
    }

    pub fn serialise(&self) -> Vec<u8> {
        let mut res = Vec::new();
        res.extend_from_slice(&self.phys_addr.to_le_bytes());
        res.extend_from_slice(&self.virt_addr.to_le_bytes());
        res.extend_from_slice(&self.len.to_le_bytes());
        res.extend_from_slice(&self.actual_len.to_le_bytes());
        // reservation for CRC
        res.extend_from_slice(&0u32.to_le_bytes());
        res.extend_from_slice(&self.codes.to_le_bytes());
        res
    }

    pub const fn get_region_size() -> usize {
        std::mem::size_of::<u32>() * 6
    }

    pub fn is_enabled(&self) -> bool {
        self.len & Self::ENABLE_MASK != 0
    }

    pub fn is_device(&self) -> bool {
        self.len & Self::DEVICE_MASK != 0
    }

    pub fn has_virt(&self) -> bool {
        self.len & Self::VIRTUAL_MASK != 0
    }

    pub fn has_phys(&self) -> bool {
        self.len & Self::PHYSICAL_MASK != 0
    }

    pub fn display(&self, indent: usize) {
        let indent = "\t".repeat(indent).to_string();
        if let Some(name) = &self.name {
            println!("{}Name: {}", indent, name);
        }
        let len = 1 << (((self.len & Self::LEN_MASK) >> Self::LEN_SHIFT) + 1);
        if self.has_phys() {
            println!("{}Physical: 0x{:x} -> 0x{:x}", indent, self.phys_addr, self.phys_addr + len);
        }
        if self.has_virt() {
            println!("{}Load: 0x{:x} -> 0x{:x}", indent, self.virt_addr, self.virt_addr + len);
        }
        println!("{}Actual Size: 0x{:x}", indent, self.actual_len);
        let physical = if self.len & Self::PHYSICAL_MASK != 0 {
            "p"
        } else {
            "-"
        };
        let virt = if self.len & Self::VIRTUAL_MASK != 0 {
            "v"
        } else {
            "-"
        };
        let device = if self.len & Self::DEVICE_MASK != 0 {
            "d"
        } else {
            "-"
        };
        let zero = if self.len & Self::ZERO_MASK != 0 {
            "z"
        } else {
            "-"
        };
        let attr = ((self.len & Self::PERM_MASK) >> Self::PERM_SHIFT) as u8;
        let attr = SectionAttr::new(true, attr & SectionAttr::WRITE_MASK != 0, attr & SectionAttr::EXEC_MASK != 0);
        println!("{}Attributes: {}{}{}{}{}", indent, zero, attr, device, physical, virt);
        println!("{}Codes: 0x{:x}", indent, self.codes);
    }
}
