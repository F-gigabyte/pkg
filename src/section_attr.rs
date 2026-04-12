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

/// Section attribute
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionAttr {
    attr: u8,
}

impl SectionAttr {
    /// Read mask
    pub const READ_MASK: u8 = 0b100;
    /// Write mask
    pub const WRITE_MASK: u8 = 0b010;
    /// Execute mask
    pub const EXEC_MASK: u8 = 0b001;

    /// Creates a new `SectionAttr`
    /// `read` is whether reading is allowed  
    /// `write` is whether writing is allowed  
    /// `exec` is whether executing is allowed
    pub fn new(read: bool, write: bool, exec: bool) -> Self {
        let mut attr = 0;
        if read {
            attr |= Self::READ_MASK;
        }
        if write {
            attr |= Self::WRITE_MASK;
        }
        if exec {
            attr |= Self::EXEC_MASK;
        }
        Self {
            attr
        }
    }

    /// Whether reading is allowed
    pub fn read(&self) -> bool {
        self.attr & Self::READ_MASK != 0
    }
    
    /// Whether writing is allowed
    pub fn write(&self) -> bool {
        self.attr & Self::WRITE_MASK != 0
    }
    
    /// Whether executing is allowed
    pub fn exec(&self) -> bool {
        self.attr & Self::EXEC_MASK != 0
    }

    /// Updates read flag  
    /// `state` is what to update the flag to
    pub fn set_read(&mut self, state: bool) {
        if state {
            self.attr |= Self::READ_MASK;
        } else {
            self.attr &= !Self::READ_MASK;
        }
    }
    
    /// Updates write flag  
    /// `state` is what to update the flag to
    pub fn set_write(&mut self, state: bool) {
        if state {
            self.attr |= Self::WRITE_MASK;
        } else {
            self.attr &= !Self::WRITE_MASK;
        }
    }
    
    /// Updates execute flag  
    /// `state` is what to update the flag to
    pub fn set_exec(&mut self, state: bool) {
        if state {
            self.attr |= Self::EXEC_MASK;
        } else {
            self.attr &= !Self::EXEC_MASK;
        }
    }
}

/// Formats the section attribute
impl fmt::Display for SectionAttr {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.read() {
            write!(fmt, "r")?;
        } else {
            write!(fmt, "-")?;
        }
        if self.write() {
            write!(fmt, "w")?;
        } else {
            write!(fmt, "-")?;
        }
        if self.exec() {
            write!(fmt, "x")?;
        } else {
            write!(fmt, "-")?;
        }
        Ok(())
    }
}
