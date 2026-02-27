use crate::section_attr::SectionAttr;

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
        Self { 
            phys_addr: 0, 
            virt_addr: 0, 
            len: 0 
        }
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

    pub fn display(&self, indent: usize) {
        let indent = "\t".repeat(indent).to_string();
        println!("{}Physical: 0x{:x} -> 0x{:x}", indent, self.phys_addr, self.phys_addr + self.len);
        let virt_addr = self.virt_addr & Self::ADDR_MASK;
        println!("{}Load: 0x{:x} -> 0x{:x}", indent, virt_addr, virt_addr + self.len);
        let attr = ((self.virt_addr & Self::PERM_MASK) >> Self::PERM_SHIFT) as u8;
        let attr = SectionAttr::new(attr & SectionAttr::READ_MASK != 0, attr & SectionAttr::WRITE_MASK != 0, attr & SectionAttr::EXEC_MASK != 0);
        println!("{}Perm: {}", indent, attr);
        if self.virt_addr & Self::DEVICE_MASK != 0 {
            println!("{}Device Memory", indent);
        }
        if self.virt_addr & Self::ZERO_MASK != 0 {
            println!("{}Zero Init", indent);
        }
    }
}

