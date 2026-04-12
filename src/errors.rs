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

use crate::{Endpoint, section_attr::SectionAttr};

/// Different Pkg Errors
#[derive(Debug)]
pub enum PkgError {
    /// Error reading file
    ReadError {
        file: String
    },
    /// File doesn't exist
    NoFile {
        file: String
    },
    /// There are multiple files matching a condition
    MultipleFiles {
        file: String,
        files: Vec<String>
    },
    /// Error writing to a file
    WriteError {
        file: String
    },
    /// Error creating a directory
    MkdirError,
    /// Error parsing a file
    ParseError {
        file: String
    },
    /// No string table in an elf file
    NoStringTable {
        file: String
    },
    /// Elf file is non-relocatable
    NonRelocatable {
        file: String
    },
    /// No space for allocating a region
    NoSpace {
        name: String, 
        region: String
    },
    /// Too many sections in an elf file
    TooManySections {
        name: String
    },
    /// Invalid arguments were specified
    InvalidArgs {
        name: String
    },
    /// No kernel entry point
    NoKernelEntry,
    /// No kernel stack
    NoKernelStack,
    /// No program entry point
    NoProgramEntry {
        name: String
    },
    /// No program stack
    NoProgramStack {
        name: String
    },
    /// Error running command
    CmdError {
        cmd: String
    },
    /// Invalid region permissions specified
    InvalidRegionPermissions {
        name: String, 
        region: String, 
        flags: SectionAttr
    },
    /// Program doesn't exist
    NoProgram {
        name: String
    },
    /// Device doesn't exist
    InvalidDriver {
        name: String, 
        driver: String
    },
    /// Device has already been allocated
    DriverTaken {
        name: String, 
        driver: String
    },
    /// Invalid pins were specified
    InvalidPins {
        name: String,
        driver: String,
        pins: Vec<u8>
    },
    /// Pins have already been allocated
    PinsTaken {
        name: String,
        driver: String,
        pins: Vec<u8>
    },
    /// This program has the same name as a previous program
    RepeatedProgram {
        name: String
    },
    /// Queues don't exist for the following endpoints
    MissingSyncQueues {
        queues: Vec<Endpoint>
    },
    /// Queues don't exist for the following asynchronous endpoints
    MissingAsyncQueues {
        queues: Vec<Endpoint>
    },
    /// Invalid asynchronous message length was specified
    BadAsyncMessageLen {
        len: usize
    }
}

/// Formats the `PkgError`
impl fmt::Display for PkgError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PkgError::InvalidArgs { name } => {
                write!(fmt, "Usage: {} [-r] config -o outfile", name)
            },
            PkgError::ReadError { file } => {
                write!(fmt, "Error reading file '{}'.", file)
            },
            PkgError::NoFile { file } => {
                write!(fmt, "Unable to locate a suitable file for '{}'.", file)
            },
            PkgError::MultipleFiles { file, files } => {
                assert!(files.len() >= 2);
                write!(fmt, "Multiple candidates are suitable for '{}' of ", file)?;
                for i in 0..files.len() - 2 {
                    write!(fmt, "'{}', ", files[i])?;
                }
                write!(fmt, "'{}' and '{}'.", files[files.len() - 2], files[files.len() - 1])
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
            PkgError::MkdirError => {
                write!(fmt, "Error creating root temporary directory.")
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
            PkgError::DriverTaken { name, driver } => {
                write!(fmt, "Repeated driver for {} of '{}'.", name, driver)
            },
            PkgError::InvalidPins { name, driver, pins } => {
                assert!(pins.len() > 0);
                if pins.len() == 1 {
                    write!(fmt, "Invalid pin for {} with driver {} of '{}'.", name, driver, pins.first().unwrap())
                } else {
                    assert!(pins.len() >= 2);
                    write!(fmt, "Invalid pins for {} with driver {} of ", name, driver)?;
                    for i in 0..pins.len() - 2 {
                        write!(fmt, "'{}', ", pins[i])?;
                    }
                    write!(fmt, "'{}' and '{}'.", pins[pins.len() - 2], pins[pins.len() - 1])
                }
            },
            PkgError::PinsTaken { name, driver, pins } => {
                assert!(pins.len() > 0);
                if pins.len() == 1 {
                    write!(fmt, "Repeated pin for {} with driver {} of '{}'.", name, driver, pins.first().unwrap())
                } else {
                    assert!(pins.len() >= 2);
                    write!(fmt, "Repeated pins for {} with driver {} of ", name, driver)?;
                    for i in 0..pins.len() - 2 {
                        write!(fmt, "'{}', ", pins[i])?;
                    }
                    write!(fmt, "'{}' and '{}'.", pins[pins.len() - 2], pins[pins.len() - 1])
                }
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
