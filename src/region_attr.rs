use std::fmt;

use crate::section_attr::SectionAttr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionAttr {
    RX = 0b101,
    R = 0b100,
    RW = 0b110
}

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

impl fmt::Display for RegionAttr {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::RX => write!(fmt, "rx"),
            Self::R => write!(fmt, "r"),
            Self::RW => write!(fmt, "rw")
        }
    }
}

