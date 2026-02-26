use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionAttr {
    attr: u8,
}

impl SectionAttr {
    pub const READ_MASK: u8 = 0b100;
    pub const WRITE_MASK: u8 = 0b010;
    pub const EXEC_MASK: u8 = 0b001;

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

    pub fn read(&self) -> bool {
        self.attr & Self::READ_MASK != 0
    }
    
    pub fn write(&self) -> bool {
        self.attr & Self::WRITE_MASK != 0
    }
    
    pub fn exec(&self) -> bool {
        self.attr & Self::EXEC_MASK != 0
    }

    pub fn set_read(&mut self, state: bool) {
        if state {
            self.attr |= Self::READ_MASK;
        } else {
            self.attr &= !Self::READ_MASK;
        }
    }
    
    pub fn set_write(&mut self, state: bool) {
        if state {
            self.attr |= Self::WRITE_MASK;
        } else {
            self.attr &= !Self::WRITE_MASK;
        }
    }
    
    pub fn set_exec(&mut self, state: bool) {
        if state {
            self.attr |= Self::EXEC_MASK;
        } else {
            self.attr &= !Self::EXEC_MASK;
        }
    }
}

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


