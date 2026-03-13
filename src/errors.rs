use std::fmt;

use crate::{Endpoint, section_attr::SectionAttr};

#[derive(Debug)]
pub enum PkgError {
    ReadError {
        file: String
    },
    WriteError {
        file: String
    },
    MkdirError {
        path: String
    },
    ParseError {
        file: String
    },
    NoStringTable {
        file: String
    },
    NonRelocatable {
        file: String
    },
    NoSpace {
        name: String, 
        region: String
    },
    TooManySections {
        name: String
    },
    InvalidArgs {
        name: String
    },
    NoKernelEntry,
    NoKernelStack,
    NoProgramEntry {
        name: String
    },
    NoProgramStack {
        name: String
    },
    CmdError {
        cmd: String
    },
    InvalidRegionPermissions {
        name: String, 
        region: String, 
        flags: SectionAttr
    },
    NoProgram {
        name: String
    },
    InvalidDriver {
        name: String, 
        driver: String
    },
    RepeatedProgram {
        name: String
    },
    MissingSyncQueues {
        queues: Vec<Endpoint>
    },
    MissingAsyncQueues {
        queues: Vec<Endpoint>
    },
    BadAsyncMessageLen {
        len: usize
    }
}

impl fmt::Display for PkgError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PkgError::InvalidArgs { name } => {
                write!(fmt, "Usage: {} [-r] config -o outfile", name)
            },
            PkgError::ReadError { file } => {
                write!(fmt, "Error reading file '{}'.", file)
            },
            PkgError::ParseError { file } => {
                write!(fmt, "Error parsing file '{}'.", file)
            },
            PkgError::NoStringTable { file } => {
                write!(fmt, "{} has no string table.", file)
            },
            PkgError::NonRelocatable { file } => {
                write!(fmt, "{} is non relocatable.", file)
            },
            PkgError::NoSpace { name, region } => {
                write!(fmt, "No space for {} region '{}'.", name, region)
            },
            PkgError::CmdError { cmd } => {
                write!(fmt, "Error running command '{}'.", cmd)
            },
            PkgError::TooManySections { name } => {
                write!(fmt, "{} has too many sections.", name)
            },
            PkgError::NoKernelEntry => {
                write!(fmt, "No kernel entry address.")
            },
            PkgError::NoKernelStack => {
                write!(fmt, "No kernel entry stack.")
            },
            PkgError::WriteError { file } => {
                write!(fmt, "Error writing to file '{}'.", file)
            },
            PkgError::MkdirError { path } => {
                write!(fmt, "Error creating directory '{}'.", path)
            },
            PkgError::NoProgramEntry { name } => {
                write!(fmt, "No entry address for program '{}'.", name)
            },
            PkgError::NoProgramStack { name } => {
                write!(fmt, "No stack for program '{}'.", name)
            },
            PkgError::NoProgram { name } => {
                write!(fmt, "No program with name '{}'.", name)
            },
            PkgError::InvalidRegionPermissions { name, region, flags } => {
                write!(fmt, "Invalid region permissions for {} region {} of '{}'.", name, region, flags)
            },
            PkgError::InvalidDriver { name, driver } => {
                write!(fmt, "Invalid driver for {} of '{}'.", name, driver)
            },
            PkgError::RepeatedProgram { name } => {
                write!(fmt, "Repeated program with name '{}'.", name)
            },
            PkgError::MissingSyncQueues { queues } => {
                let mut msg = "Missing sync queues for sync endpoints ".to_string();
                for i in 0..queues.len() - 1 {
                    msg = format!("{} {}[{}]", msg, queues[i].name, queues[i].queue);
                }
                msg = format!("{} {}[{}]", msg, queues.last().unwrap().name, queues.last().unwrap().queue);
                write!(fmt, "{}", msg)
            },
            PkgError::MissingAsyncQueues { queues } => {
                let mut msg = "Missing async queues for async endpoints ".to_string();
                for i in 0..queues.len() - 1 {
                    msg = format!("{} {}[{}]", msg, queues[i].name, queues[i].queue);
                }
                msg = format!("{} {}[{}]", msg, queues.last().unwrap().name, queues.last().unwrap().queue);
                write!(fmt, "{}", msg)
            },
            PkgError::BadAsyncMessageLen { len } => {
                write!(fmt, "Bad asynchronous message length of {} (must be multiple of 4).", len)
            },
        }
    }
}
