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

use std::fmt;

use crate::section_attr::SectionAttr;

/// Region attributes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionAttr {
    /// Read and Execute
    RX = 0b01,
    /// Read Only
    R = 0b00,
    /// Read and Write
    RW = 0b10
}

/// Attempts to convert from a `SectionAttr` to a `RegionAttr`
impl TryFrom<SectionAttr> for RegionAttr {
    type Error = SectionAttr;
    fn try_from(value: SectionAttr) -> Result<Self, Self::Error> {
        if value.read() {
            if value.write() {
                Ok(Self::RW)
            } else if value.exec() {
                Ok(Self::RX)
            } else {
                Ok(Self::R)
            }
        } else {
            Err(value)
        }
    }
}

/// Formats the region attribute
impl fmt::Display for RegionAttr {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::RX => write!(fmt, "rx"),
            Self::R => write!(fmt, "r"),
            Self::RW => write!(fmt, "rw")
        }
    }
}
