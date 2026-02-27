use std::fs;

use object::read::elf::ElfFile32;
use serde::Deserialize;

use crate::errors::PkgError;

#[derive(Deserialize)]
pub struct FileConfig {
    pub async_message_len: u32,
    pub kernel: KernelConfig,
    pub programs: Vec<ProgramConfig>
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
    pub debug_src: String,
    pub release_src: String,
}

#[derive(Debug, Deserialize, Hash, PartialEq, Eq, Clone)]
pub struct Endpoint {
    pub name: String,
    pub queue: u32
}

#[derive(Deserialize)]
pub struct ProgramConfig {
    pub name: String,
    pub priority: u8,
    pub driver: u16,
    pub debug_src: String,
    pub release_src: String,
    pub num_sync_queues: u32,
    pub async_queues: Vec<usize>,
    pub sync_endpoints: Vec<Endpoint>,
    pub async_endpoints: Vec<Endpoint>
}

pub struct LoadedConfig<'a> {
    pub filename: String,
    pub data: ElfFile32<'a>,
}
