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

use std::fs;

use object::read::elf::ElfFile32;
use serde::Deserialize;

use crate::errors::PkgError;

/// Pkg TOML configuration file format
#[derive(Deserialize)]
pub struct FileConfig {
    /// Asynchronous queue message length
    pub async_message_len: u32,
    /// Path to the debug directory
    pub debug_path: String,
    /// Path to the release directory
    pub release_path: String,
    /// Kernel configuration
    pub kernel: KernelConfig,
    /// Program configuration
    pub programs: Vec<ProgramConfig>,
}

impl FileConfig {
    /// Parses the file  
    /// `file` is the file to parse
    pub fn parse(file: &str) -> Result<Self, PkgError> {
        let config = fs::read_to_string(file).map_err(|_| 
            PkgError::ReadError {
                file: file.to_string() 
            }
        )?;
        toml::from_str::<FileConfig>(&config).map_err(|_| 
            PkgError::ParseError {
                file: file.to_string()
            }
        )
    }
}

/// Kernel configuration
#[derive(Deserialize)]
pub struct KernelConfig {
    /// Kernel executable name
    pub exec: String,
}

/// Endpoint configuration
#[derive(Debug, Deserialize, Hash, PartialEq, Eq, Clone)]
pub struct Endpoint {
    /// Program name to send to
    pub name: String,
    /// The queue to use
    pub queue: u8
}

/// Program configuration
#[derive(Deserialize)]
pub struct ProgramConfig {
    /// Name of the program
    pub name: String,
    /// Program executable name
    pub exec: String,
    /// Program priority
    pub priority: u8,
    /// Program's device to allocate
    pub driver: Option<String>,
    /// Program's pins to allocate
    pub pins: Option<Vec<u8>>,
    /// Number of synchronous queues this program has
    pub num_sync_queues: u8,
    /// The sizes in number of messages of this program's asynchronous queues
    pub async_queues: Vec<usize>,
    /// The program's synchronous endpoints
    pub sync_endpoints: Vec<Endpoint>,
    /// The program's asynchronous endpoints
    pub async_endpoints: Vec<Endpoint>,
    /// The number of notifier queues this program has
    pub num_notifiers: u8,
    /// The hamming code block length in bytes divided by 4)
    pub block_len: u32,
}

pub struct LoadedConfig<'a> {
    /// The file name
    pub filename: String,
    /// The program's device
    pub driver: u16,
    /// The program's pins to allocate
    pub pins: Option<Vec<u8>>,
    /// The program's hamming code block length
    pub block_len: u32,
    /// The loaded program elf file
    pub data: ElfFile32<'a>,
}
