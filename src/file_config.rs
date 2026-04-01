use std::fs;

use object::read::elf::ElfFile32;
use serde::Deserialize;

use crate::errors::PkgError;

#[derive(Deserialize)]
pub struct FileConfig {
    pub async_message_len: u32,
    pub debug_path: String,
    pub release_path: String,
    pub kernel: KernelConfig,
    pub programs: Vec<ProgramConfig>,
}

impl FileConfig {
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

#[derive(Deserialize)]
pub struct KernelConfig {
    pub exec: String,
}

#[derive(Debug, Deserialize, Hash, PartialEq, Eq, Clone)]
pub struct Endpoint {
    pub name: String,
    pub queue: u8
}

#[derive(Deserialize)]
pub struct ProgramConfig {
    pub name: String,
    pub exec: String,
    pub priority: u8,
    pub driver: Option<String>,
    pub pins: Option<Vec<u8>>,
    pub num_sync_queues: u8,
    pub async_queues: Vec<usize>,
    pub sync_endpoints: Vec<Endpoint>,
    pub async_endpoints: Vec<Endpoint>,
    pub num_notifiers: u8,
    pub block_len: u32,
}

pub struct LoadedConfig<'a> {
    pub filename: String,
    pub driver: u16,
    pub pins: Option<Vec<u8>>,
    pub block_len: u32,
    pub data: ElfFile32<'a>,
}
