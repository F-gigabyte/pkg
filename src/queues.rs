use std::{cmp::Ordering, collections::{HashMap, HashSet}, fs, mem, path::Path, process::Command};

use crate::{allocs::AllocInfo, cmds::check_cmd, errors::PkgError, file_config::{Endpoint, ProgramConfig}};

const ENDPOINT_SIZE: usize = 4;

// length of sync queue in bytes
const SYNC_QUEUE_SIZE: usize = 16;

// length of async queue in bytes
const ASYNC_QUEUE_SIZE: usize = 28;

// message header length in bytes
const MESSAGE_HEADER_SIZE: usize = 12;

pub struct AsyncQueue {
    pub buffer: u32,
    pub buffer_len: u32,
    pub message_len: u32,
}

impl AsyncQueue {
    pub fn serialise(&self) -> Vec<u8> {
        let mut res = Vec::new();
        res.extend_from_slice(&self.buffer.to_le_bytes());
        res.extend_from_slice(&self.buffer_len.to_le_bytes());
        res.extend_from_slice(&self.message_len.to_le_bytes());
        res.extend_from_slice(&0u32.to_le_bytes());
        res.extend_from_slice(&0u32.to_le_bytes());
        res.extend_from_slice(&0u32.to_le_bytes());
        res.extend_from_slice(&0u32.to_le_bytes());
        res
    }
}

pub struct QueueRequirements {
    available_sync_queues: HashSet<Endpoint>,
    available_async_queues: HashSet<Endpoint>,
    needed_sync_endpoints: HashSet<Endpoint>,
    needed_async_endpoints: HashSet<Endpoint>,
    queues: Queues,
}

impl QueueRequirements {
    pub fn new(message_len: usize) -> Self {
        Self {
            available_async_queues: HashSet::new(),
            available_sync_queues: HashSet::new(),
            needed_sync_endpoints: HashSet::new(),
            needed_async_endpoints: HashSet::new(),
            queues: Queues::new(message_len),
        }
    }

    pub fn add_program_queues(&mut self, program: &mut ProgramConfig) {
        for i in 0..program.num_sync_queues {
            self.available_sync_queues.insert(Endpoint {
                name: program.name.to_string(),
                queue: i
            });

            self.queues.sync_queue_offsets.insert(
                Endpoint {
                    name: program.name.to_string(),
                    queue: i
                },
                self.queues.sync_queues_size
            );
            self.queues.sync_queues_size += SYNC_QUEUE_SIZE;
        }
        for i in 0..program.async_queues.len() {
            self.available_async_queues.insert(Endpoint {
                name: program.name.to_string(),
                queue: i as u32
            });

            self.queues.async_queue_offsets.insert(
                Endpoint {
                    name: program.name.to_string(),
                    queue: i as u32
                },
                self.queues.async_queues_size
            );

            self.queues.async_queues_size += ASYNC_QUEUE_SIZE;
            self.queues.messages_offsets.insert(
                Endpoint {
                    name: program.name.to_string(),
                    queue: i as u32
                },
                self.queues.messages_size
            );
            self.queues.messages_size += program.async_queues[i] * (self.queues.message_len + MESSAGE_HEADER_SIZE);
        }
        self.queues.sync_endpoints_offsets.insert(program.name.to_string(), self.queues.sync_endpoints_size);
        for endpoint in &program.sync_endpoints {
            self.needed_sync_endpoints.insert(endpoint.clone());
        }
        // endpoint is pointer to queue
        self.queues.sync_endpoints_size += program.sync_endpoints.len() * ENDPOINT_SIZE;

        self.queues.async_endpoints_offsets.insert(program.name.to_string(), self.queues.async_endpoints_size);
        for endpoint in &program.async_endpoints {
            self.needed_async_endpoints.insert(endpoint.clone());
        }
        self.queues.async_endpoints_size += program.async_endpoints.len() * ENDPOINT_SIZE;
        
        self.queues.sync_endpoints.insert(program.name.to_string(), mem::replace(&mut program.sync_endpoints, Vec::new()));
        self.queues.async_endpoints.insert(program.name.to_string(), mem::replace(&mut program.async_endpoints, Vec::new()));
        self.queues.async_queues.insert(program.name.to_string(), mem::replace(&mut program.async_queues, Vec::new()));
    }

    pub fn requirements_satisfied(&self) -> Result<(), PkgError> {
        let sync_missing: Vec<_> = self
            .needed_sync_endpoints
            .difference(&self.available_sync_queues)
            .map(|val| val.clone())
            .collect();
        if sync_missing.len() > 0 {
            return Err(
                PkgError::MissingSyncQueues {
                    queues: sync_missing
                }
            );
        }
        let async_missing: Vec<_> = self
            .needed_async_endpoints
            .difference(&self.available_async_queues)
            .map(|val| val.clone())
            .collect();
        if async_missing.len() > 0 {
            return Err(
                PkgError::MissingAsyncQueues {
                    queues: sync_missing
                }
            );
        }
        Ok(())
    }

    pub fn get_queues(self) -> Queues {
        self.queues
    }
}

pub struct Queues {
    pub sync_queue_offsets: HashMap<Endpoint, usize>,
    pub sync_queues_size: usize,
    pub async_queue_offsets: HashMap<Endpoint, usize>,
    pub async_queues_size: usize,
    pub sync_endpoints_offsets: HashMap<String, usize>,
    pub sync_endpoints_size: usize,
    pub async_endpoints_offsets: HashMap<String, usize>,
    pub async_endpoints_size: usize,
    pub messages_offsets: HashMap<Endpoint, usize>,
    pub messages_size: usize,
    pub sync_endpoints: HashMap<String, Vec<Endpoint>>,
    pub async_endpoints: HashMap<String, Vec<Endpoint>>,
    pub async_queues: HashMap<String, Vec<usize>>,
    pub message_len: usize
}

impl Queues {
    pub fn new(message_len: usize) -> Self {
        Self { 
            sync_queue_offsets: HashMap::new(), 
            sync_queues_size: 0, 
            async_queue_offsets: HashMap::new(), 
            async_queues_size: 0, 
            sync_endpoints_offsets: HashMap::new(),
            sync_endpoints_size: 0,
            async_endpoints_offsets: HashMap::new(),
            async_endpoints_size: 0,
            messages_offsets: HashMap::new(), 
            messages_size: 0, 
            sync_endpoints: HashMap::new(),
            async_endpoints: HashMap::new(),
            async_queues: HashMap::new(),
            message_len: message_len
        }
    }

    fn get_sync_endpoints_data(&self, alloc_info: &AllocInfo) -> Vec<u8> {
        let mut sync_endpoints: Vec<_> = self.sync_endpoints_offsets.iter().collect();
        sync_endpoints.sort_by(|a, b| -> Ordering {
            if a.1 < b.1 {
                Ordering::Less
            } else if a.1 > b.1 {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        });
        let mut sync_endpoints_bytes = Vec::new();
        for (name, _) in sync_endpoints {
            let endpoints = &self.sync_endpoints[name];
            for endpoint in endpoints {
                let queue_offset = self.sync_queue_offsets[&endpoint];
                let queue_addr = alloc_info.sync_queues_virt as u32 + queue_offset as u32;
                sync_endpoints_bytes.extend_from_slice(&queue_addr.to_le_bytes());
            }
        }
        sync_endpoints_bytes
    }

    fn get_async_endpoints_data(&self, alloc_info: &AllocInfo) -> Vec<u8> {
        let mut async_endpoints: Vec<_> = self.async_endpoints_offsets.iter().collect();
        async_endpoints.sort_by(|a, b| -> Ordering {
            if a.1 < b.1 {
                Ordering::Less
            } else if a.1 > b.1 {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        });
        let mut async_endpoints_bytes = Vec::new();
        for (name, _) in async_endpoints {
            let endpoints = &self.async_endpoints[name];
            for endpoint in endpoints {
                let queue_offset = self.async_queue_offsets[&endpoint];
                let queue_addr = alloc_info.async_queues_virt as u32 + queue_offset as u32;
                async_endpoints_bytes.extend_from_slice(&queue_addr.to_le_bytes());
            }
        }
        async_endpoints_bytes
    }

    fn get_async_queues_data(&self, alloc_info: &AllocInfo) -> Vec<u8> {
        let mut async_queues_vec: Vec<_> = self.async_queue_offsets.iter().collect();
        async_queues_vec.sort_by(|a, b| -> Ordering {
            if a.1 < b.1 {
                Ordering::Less
            } else if a.1 > b.1 {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        });
        let mut async_queues_bytes = Vec::new();
        for (name, _) in async_queues_vec {
            let messages_offset = self.messages_offsets[&name];
            let messages_addr = alloc_info.messages_virt + messages_offset;
            let buffer_len = self.async_queues[&name.name][name.queue as usize] as u32;
            let async_queue = AsyncQueue {
                buffer: messages_addr as u32,
                buffer_len,
                message_len: self.message_len as u32
            };
            async_queues_bytes.extend(async_queue.serialise());
        }
        async_queues_bytes
    }

    pub fn write_sync_enpoints_file(&self, path: &Path, alloc_info: &AllocInfo, objcopy: &str) -> Result<Option<String>, PkgError> {
        let sync_endpoints_bytes = self.get_sync_endpoints_data(alloc_info);
        if sync_endpoints_bytes.len() > 0 {
            let sync_endpoints_file_bin = path.join("sync_endpoints.bin");
            let sync_endpoints_file = path.join("sync_endpoints.o").to_string_lossy().to_string().to_string();
            fs::write(&sync_endpoints_file_bin, sync_endpoints_bytes).map_err(|_| 
                PkgError::WriteError {
                    file: sync_endpoints_file_bin.to_str().unwrap().to_string()
                }
            )?;
            let sync_endpoints_file_bin = sync_endpoints_file_bin.to_string_lossy().to_string().to_string();
            let mut cmd = Command::new(objcopy);
            cmd
                .arg("-O")
                .arg("elf32-littlearm")
                .arg("-I")
                .arg("binary")
                .arg("-B")
                .arg("arm")
                .arg(&sync_endpoints_file_bin)
                .arg(&sync_endpoints_file);
            check_cmd(cmd).map_err(|_| 
                PkgError::CmdError {
                    cmd: objcopy.to_string()
                }
            )?;
            Ok(Some(sync_endpoints_file))
        } else {
            Ok(None)
        }
    }

    pub fn write_async_endpoints_file(&self, path: &Path, alloc_info: &AllocInfo, objcopy: &str) -> Result<Option<String>, PkgError> {
        let async_endpoints_bytes = self.get_async_endpoints_data(alloc_info);
        if async_endpoints_bytes.len() > 0 {
            let async_endpoints_file_bin = path.join("async_endpoints.bin");
            let async_endpoints_file = path.join("async_endpoints.o").to_string_lossy().to_string().to_string();
            fs::write(&async_endpoints_file_bin, async_endpoints_bytes).map_err(|_| 
                PkgError::WriteError {
                    file: async_endpoints_file_bin.to_str().unwrap().to_string()
                }
            )?;
            let async_endpoints_file_bin = async_endpoints_file_bin.to_string_lossy().to_string().to_string();
            let mut cmd = Command::new(objcopy);
            cmd
                .arg("-O")
                .arg("elf32-littlearm")
                .arg("-I")
                .arg("binary")
                .arg("-B")
                .arg("arm")
                .arg(&async_endpoints_file_bin)
                .arg(&async_endpoints_file);
            check_cmd(cmd).map_err(|_| 
                PkgError::CmdError {
                    cmd: objcopy.to_string()
                }
            )?;
            Ok(Some(async_endpoints_file))
        } else {
            Ok(None)
        }
    }

    pub fn write_async_queues_file(&self, path: &Path, alloc_info: &AllocInfo, objcopy: &str) -> Result<Option<String>, PkgError> {
        let async_queues_bytes = self.get_async_queues_data(alloc_info);
        if async_queues_bytes.len() > 0 {
            let async_queues_file_bin = path.join("async_queues.bin");
            let async_queues_file = path.join("async_queues.o").to_string_lossy().to_string().to_string();
            fs::write(&async_queues_file_bin, async_queues_bytes).map_err(|_| 
                PkgError::WriteError {
                    file: async_queues_file_bin.to_str().unwrap().to_string()
                }
            )?;
            let async_queues_file_bin = async_queues_file_bin.to_string_lossy().to_string().to_string();
            let mut cmd = Command::new(&objcopy);
            cmd
                .arg("-O")
                .arg("elf32-littlearm")
                .arg("-I")
                .arg("binary")
                .arg("-B")
                .arg("arm")
                .arg(&async_queues_file_bin)
                .arg(&async_queues_file);
            check_cmd(cmd).map_err(|_| 
                PkgError::CmdError { 
                    cmd: objcopy.to_string()
                }
            )?;
            Ok(Some(async_queues_file))
        } else {
            Ok(None)
        }
    }
}
