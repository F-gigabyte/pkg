/* 
 * Copyright 2026 Fraser Griffin
 *
 * This file is part of Pkg.
 *
 * Pkg is free software: you can redistribute it and/or modify it under 
 * the terms of the GNU General Public License as published by the Free Software Foundation, 
 * either version 3 of the License, or (at your option) any later version.
 *
 * Pkg is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; 
 * without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. 
 * See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public License along with Pkg. 
 * If not, see <https://www.gnu.org/licenses/>. 
 * 
 */

use crate::section_attr::SectionAttr;

/// A program region
#[derive(Debug)]
pub struct Region {
    // name is just there for reference but doesn't end up in the final binary
    /// Region name
    pub name: Option<String>,
    /// Region's flash address
    pub phys_addr: u32,
    /// Region's virtual address
    pub virt_addr: u32,
    /// Region's length and flags
    pub len: u32,
    /// Region's actual length (unpadded)
    pub actual_len: u32,
    /// Address of region's hamming codes
    pub codes: u32,
}

impl Region {
    // len mask
    /// Shift to enable the region
    pub const ENABLE_SHIFT: usize = 0;
    /// Shift to specify this region is loaded into RAM
    pub const VIRTUAL_SHIFT: usize = 1;
    /// Shift to specify this region exists in flash
    pub const PHYSICAL_SHIFT: usize = 2;
    /// Shift to specify this region is memory mapped IO
    pub const DEVICE_SHIFT: usize = 3;
    /// Region permission shift
    pub const PERM_SHIFT: usize = 4;
    /// Shift to specify region should be zeroed
    pub const ZERO_SHIFT: usize = 6;
    /// Len shift
    pub const LEN_SHIFT: usize = 16;

    /// Mask to enable the region
    pub const ENABLE_MASK: u32 = 1 << Self::ENABLE_SHIFT;
    /// Mask to specify this region is loaded into RAM
    pub const VIRTUAL_MASK: u32 = 1 << Self::VIRTUAL_SHIFT;
    /// Mask to specify this region exists in flash
    pub const PHYSICAL_MASK: u32 = 1 << Self::PHYSICAL_SHIFT;
    /// Mask to specify this region is memory mapped IO
    pub const DEVICE_MASK: u32 = 1 << Self::DEVICE_SHIFT;
    /// Region permission mask
    pub const PERM_MASK: u32 = 0x3 << Self::PERM_SHIFT;
    /// Mask to specify region should be zeroed
    pub const ZERO_MASK: u32 = 1 << Self::ZERO_SHIFT;
    /// Len mask
    pub const LEN_MASK: u32 = 0xffff << Self::LEN_SHIFT;

    /// Empty region
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

    /// Converts the region into a byte stream
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

    /// Gets the byte size of a serialised `Region` object
    pub const fn get_region_size() -> usize {
        std::mem::size_of::<u32>() * 6
    }

    /// Whether the region is enabled
    pub fn is_enabled(&self) -> bool {
        self.len & Self::ENABLE_MASK != 0
    }

    /// Whether the region is memory mapped IO
    pub fn is_device(&self) -> bool {
        self.len & Self::DEVICE_MASK != 0
    }

    /// Whether the region is loaded into RAM
    pub fn has_virt(&self) -> bool {
        self.len & Self::VIRTUAL_MASK != 0
    }

    /// Whether the region exists in flash
    pub fn has_phys(&self) -> bool {
        self.len & Self::PHYSICAL_MASK != 0
    }

    /// Formats the region in a human readable format  
    /// `indent` is the indentation level to use on top of any further indentation
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
