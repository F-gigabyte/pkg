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

/// Kernel driver arguments
#[repr(C)]
#[derive(Debug)]
pub struct DriverArgs {
    /// GPIO pin function selects
    pub pin_func: [u32; 4],
    /// GPIO pad arguments
    pub pads: [u32; 2],
    /// Which devices should be reset
    pub resets: u32
}

/// Disable the pad
pub const PAD_DISABLE: u32 = 0;
/// Pad should be setup as normal
pub const PAD_NORMAL: u32 = 1;
/// Pad should be set up for analog input
pub const PAD_ANALOG: u32 = 2;
/// Pad should enable pull up
pub const PAD_PULL_UP: u32 = 3;

impl DriverArgs {
    /// Creates a new `DriverArgs`  
    /// Provides driver arguments for all kernel drivers
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

    /// Converts the driver arguments into a byte stream
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
