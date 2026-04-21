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

use std::{cmp::Ordering, collections::{HashMap, HashSet}, fs, mem, process::Command};

use tempfile::TempDir;

use crate::{allocs::AllocInfo, cmds::check_cmd, errors::PkgError, file_config::{Endpoint, ProgramConfig}};

/// Size of an endpoint in bytes
const ENDPOINT_SIZE: usize = 4;

/// Size of a synchronous queue in bytes
const SYNC_QUEUE_SIZE: usize = 16;

/// Size of an asynchronous queue in bytes
const ASYNC_QUEUE_SIZE: usize = 28;

/// Message header size in bytes
const MESSAGE_HEADER_SIZE: usize = 16;

/// Message header alignment in bytes
const MESSAGE_HEADER_ALIGNMENT: usize = 4;

/// An asynchronous queue
struct AsyncQueue {
    /// Asynchronous queue buffer address
    pub buffer: u32,
    /// Asynchronous queue buffer length in number of messages
    pub buffer_len: u32,
    /// Asynchronous queue message length in bytes
    pub message_len: u32,
}

impl AsyncQueue {
    /// Converts the asynchronous queue into a byte stream
    fn serialise(&self) -> Vec<u8> {
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

/// Represents the queue requirements
pub struct QueueRequirements {
    /// The synchronous queues available
    available_sync_queues: HashSet<Endpoint>,
    /// The asynchronous queues available
    available_async_queues: HashSet<Endpoint>,
    /// The needed synchronous queues
    needed_sync_endpoints: HashSet<Endpoint>,
    /// The needed asynchronous queues
    needed_async_endpoints: HashSet<Endpoint>,
    /// Queue data
    queues: Queues,
}

impl QueueRequirements {
    /// Creates a new `QueueRequirements`  
    /// `message_len` is the message length for asynchronous queues
    pub fn new(message_len: usize) -> Self {
        Self {
            available_async_queues: HashSet::new(),
            available_sync_queues: HashSet::new(),
            needed_sync_endpoints: HashSet::new(),
            needed_async_endpoints: HashSet::new(),
            queues: Queues::new(message_len),
        }
    }

    /// Adds a program's queue and endpoint data to the queue requirements  
    /// `program` is the program whose queue data should be added
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
                queue: i as u8
            });

            self.queues.async_queue_offsets.insert(
                Endpoint {
                    name: program.name.to_string(),
                    queue: i as u8
                },
                self.queues.async_queues_size
            );

            self.queues.async_queues_size += ASYNC_QUEUE_SIZE;
            self.queues.messages_offsets.insert(
                Endpoint {
                    name: program.name.to_string(),
                    queue: i as u8
                },
                self.queues.messages_size
            );
            let message_size = ((self.queues.message_len + MESSAGE_HEADER_SIZE + MESSAGE_HEADER_ALIGNMENT - 1) / MESSAGE_HEADER_ALIGNMENT) * MESSAGE_HEADER_ALIGNMENT;
            self.queues.messages_size += program.async_queues[i] * message_size;
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
        self.queues.notifier_offsets.insert(program.name.to_string(), self.queues.notifier_size);
        self.queues.notifier_size += (program.num_notifiers as usize) * SYNC_QUEUE_SIZE;
    }

    /// Checks whether the queue requirements have been satisfied
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

    /// Returns the queue data for later, consuming `self`
    pub fn get_queues(self) -> Queues {
        self.queues
    }
}

/// Queue data
pub struct Queues {
    /// Synchronous queue offsets
    pub sync_queue_offsets: HashMap<Endpoint, usize>,
    /// Synchronous queues size in bytes
    pub sync_queues_size: usize,
    /// Asynchronous queue offsets
    pub async_queue_offsets: HashMap<Endpoint, usize>,
    /// Asynchronous queues size in bytes
    pub async_queues_size: usize,
    /// Synchronous endpoints offsets
    pub sync_endpoints_offsets: HashMap<String, usize>,
    /// Synchronous endpoints size in bytes
    pub sync_endpoints_size: usize,
    /// Asynchronous endpoints offsets
    pub async_endpoints_offsets: HashMap<String, usize>,
    /// Asynchronous endpoints size in bytes
    pub async_endpoints_size: usize,
    /// Asynchronous queue message offsets
    pub messages_offsets: HashMap<Endpoint, usize>,
    /// Asynchronous queue messages size in bytes
    pub messages_size: usize,
    /// Synchronous endpoints
    pub sync_endpoints: HashMap<String, Vec<Endpoint>>,
    /// Asynchronous endpoints
    pub async_endpoints: HashMap<String, Vec<Endpoint>>,
    /// Asynchronous queues
    pub async_queues: HashMap<String, Vec<usize>>,
    /// Asynchronous Message length in bytes
    pub message_len: usize,
    /// Notifier queue offsets
    pub notifier_offsets: HashMap<String, usize>,
    /// Notifier queues size in bytes
    pub notifier_size: usize
}

impl Queues {
    /// Creates a new `Queues`  
    /// `message_len` is the asynchronous queue message length in bytes
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
            message_len: message_len,
            notifier_offsets: HashMap::new(),
            notifier_size: 0
        }
    }

    /// Serialises the synchronous endpoints data into a byte stream  
    /// `alloc_info` is the allocation information
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

    /// Serialises the asynchronous endpoints data into a byte stream  
    /// `alloc_info` is the allocation information
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

    /// Serialises the asynchronous queues data into a byte stream  
    /// `alloc_info` is the allocation information
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

    /// Creates an elf object file containing the synchronous endpoints data for linking later  
    /// `root` is the root of the temporary directory  
    /// `alloc_info` is the allocation information  
    /// `objcopy` is the objcopy binary to use
    /// Returns the name of the file if successfully created, `None` if successful but no file was
    /// created and a `PkgError` on error
    pub fn write_sync_enpoints_file(&self, root: &TempDir, alloc_info: &AllocInfo, objcopy: &str) -> Result<Option<String>, PkgError> {
        let sync_endpoints_bytes = self.get_sync_endpoints_data(alloc_info);
        if sync_endpoints_bytes.len() > 0 {
            let sync_endpoints_file_bin = root.path().join("sync_endpoints.bin");
            let sync_endpoints_file = root.path().join("sync_endpoints.o").to_string_lossy().to_string().to_string();
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

    /// Creates an elf object file containing the asynchronous endpoints data for linking later  
    /// `root` is the root of the temporary directory  
    /// `alloc_info` is the allocation information  
    /// `objcopy` is the objcopy binary to use
    /// Returns the name of the file if successfully created, `None` if successful but no file was
    /// created and a `PkgError` on error
    pub fn write_async_endpoints_file(&self, root: &TempDir, alloc_info: &AllocInfo, objcopy: &str) -> Result<Option<String>, PkgError> {
        let async_endpoints_bytes = self.get_async_endpoints_data(alloc_info);
        if async_endpoints_bytes.len() > 0 {
            let async_endpoints_file_bin = root.path().join("async_endpoints.bin");
            let async_endpoints_file = root.path().join("async_endpoints.o").to_string_lossy().to_string().to_string();
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

    /// Creates an elf object file containing the asynchronous queues data for linking later  
    /// `root` is the root of the temporary directory  
    /// `alloc_info` is the allocation information  
    /// `objcopy` is the objcopy binary to use
    /// Returns the name of the file if successfully created, `None` if successful but no file was
    /// created and a `PkgError` on error
    pub fn write_async_queues_file(&self, root: &TempDir, alloc_info: &AllocInfo, objcopy: &str) -> Result<Option<String>, PkgError> {
        let async_queues_bytes = self.get_async_queues_data(alloc_info);
        if async_queues_bytes.len() > 0 {
            let async_queues_file_bin = root.path().join("async_queues.bin");
            let async_queues_file = root.path().join("async_queues.o").to_string_lossy().to_string().to_string();
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
