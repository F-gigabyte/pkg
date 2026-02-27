use std::{cmp::Ordering, collections::{HashMap, HashSet, VecDeque}, env, fs::{self, File}, io::Read, path::Path, process::{Command, exit}};

use object::{Endianness, Object, ObjectKind, ObjectSection, ObjectSymbol, SectionIndex, StringTable, elf::{SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, STT_FUNC}, read::elf::{ElfFile, FileHeader, SectionHeader}};

use ansi_term::Color::Red;

use crate::{allocs::{Alloc, MemMap, default_allocs, do_allocs}, args::Args, async_queue::AsyncQueue, cmds::check_cmd, drivers::find_driver, errors::PkgError, file_config::{Endpoint, FileConfig, LoadedConfig}, program::Program, region::Region, region_attr::RegionAttr, section_attr::SectionAttr, sections::{Section, SectionRename, create_link_file, print_renames, rename_file_sections}};

pub mod drivers;
pub mod async_queue;
pub mod section_attr;
pub mod region_attr;
pub mod endpoints;
pub mod region;
pub mod program;
pub mod errors;
pub mod allocs;
pub mod sections;
pub mod cmds;
pub mod file_config;
pub mod args;

const MIN_SIZE: u32 = 256;

// length of sync queue in bytes
const SYNC_QUEUE_SIZE: usize = 16;

// length of async queue in bytes
const ASYNC_QUEUE_SIZE: usize = 28;

// message header length in bytes
const MESSAGE_HEADER_SIZE: usize = 12;

const ENDPOINT_SIZE: usize = 4;

const BOOTLOADER_ADDR: usize = 0x10000000;
const VECTORS_ADDR: usize = 0x10000000 + 256;

const PROC_SIZE: usize = 13 * 4;

const FLASH_START: usize = 0x10000000;
const FLASH_LEN: usize = 2048 * 1024;

const RAM_START: usize = 0x20000000;
const RAM_LEN: usize = 264 * 1024;

fn get_file_regions(name: &str, file: &LoadedConfig, allocs: &mut VecDeque<Alloc>, renames: &mut HashMap<String, (Vec<SectionRename>, Vec<String>)>) -> Result<(), PkgError> {
    // check relocatable
    if file.data.kind() != ObjectKind::Relocatable {
        return Err(
            PkgError::NonRelocatable {
                file: file.filename.to_string() 
            }
        );
    }
    let is_kernel = name == "kernel";
    // get string table to get section names
    let index = file.data.elf_header().e_shstrndx(Endianness::Little);
    let string_sec = file.data.section_by_index(SectionIndex(index as usize)).map_err(|_| 
        PkgError::NoStringTable {
            file: file.filename.to_string()
        }
    )?;
    let data = string_sec.data().map_err(|_| 
        PkgError::NoStringTable {
            file: file.filename.to_string() 
        }
    )?;
    let string_table = StringTable::new(data, 0, data.len() as u64);
    let mut file_secs = Vec::new();
    let entry_addr = file.data.entry() as usize;
    
    let mut file_symbols = Vec::new();

    let mut entry_sec = None;

    for symbol in file.data.symbols() {
        let name = symbol.name().map_err(|_| 
            PkgError::NoStringTable {
                file: file.filename.to_string()
            }
        )?.to_string();
        let addr = symbol.address() as usize;
        if addr == entry_addr && symbol.elf_symbol().st_type() == STT_FUNC {
            entry_sec = symbol.section_index();
        }
        file_symbols.push(name);
    }

    for (i, sec) in file.data.elf_section_table().iter().enumerate() {
        if sec.sh_flags(Endianness::Little) & SHF_ALLOC != 0 {
            let region_name = String::from_utf8_lossy(sec.name(Endianness::Little, string_table).map_err(|_| 
                    PkgError::NoStringTable {
                        file: file.filename.to_string() 
                    }
                )?
            );
            let sec_size = sec.sh_size(Endianness::Little) as usize;
            let size = sec_size as usize;
            let addr = sec.sh_addr(Endianness::Little) as usize;
            // get section flags
            let mut flags = SectionAttr::new(true, false, false);
            if sec.sh_flags(Endianness::Little) & SHF_EXECINSTR != 0 {
                flags.set_exec(true);
            }
            if sec.sh_flags(Endianness::Little) & SHF_WRITE != 0 {
                flags.set_write(true);
            }
            let load = if sec.sh_flags(Endianness::Little) & SHF_WRITE != 0 {
                true
            } else {
                false
            };
            // rename section to be filename.section_name so can produce a linker script for it
            // later 
            file_secs.push(SectionRename {
                old_name: region_name.to_string(),
                new_name: format!("{}{}", name, region_name)
            });
            let entry_addr = if Some(SectionIndex(i)) == entry_sec {
                Some(entry_addr - addr)
            } else {
                None
            };
            let size = if name == "kernel" { size } else { size.next_power_of_two().max(MIN_SIZE as usize) };
            // put section to be allocated later
            let alloc = Alloc {
                name: name.to_string(),
                region: region_name.to_string(),
                queue: false,
                need_region: !is_kernel,
                store: region_name != ".bss",
                attr: RegionAttr::try_from(flags).map_err(|err| 
                    PkgError::InvalidRegionPermissions {
                        name: name.to_string(), 
                        region: region_name.to_string(), 
                        flags: err
                    }
                )?,
                load,
                entry_addr,
                size,
                alignment: if name == "kernel" { 4 } else { size }
            };
            let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
            allocs.insert(index, alloc);
        }
    }

    if let Some(stack) = file.data.symbol_by_name("__stack_size") {
        // if have reserved a stack, allocate this as well
        let alloc = Alloc { 
            name: name.to_string(),
            region: ".stack".to_string(), 
            queue: false,
            need_region: !is_kernel,
            store: false,
            attr: RegionAttr::RW, 
            load: true, 
            entry_addr: None,
            size: if name == "kernel" { stack.address() as usize } else { (stack.address() as usize).next_power_of_two().max(MIN_SIZE as usize) },
            alignment: if name == "kernel" { 4 } else { stack.address() as usize }
        };
        let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
        allocs.insert(index, alloc);
    }
    // add section renames to hash map
    _ = renames.insert(file.filename.to_string(), (file_secs, file_symbols));
    Ok(())
}


fn run(args: Vec<String>) -> Result<(), PkgError> {

    let args = Args::parse(&args)?;
    let config = FileConfig::parse(args.config_file)?;

    let mut sections = Vec::new();
    let mut renames = HashMap::new();
    let mut file_data = HashMap::new();
    let mut programs = HashMap::new();
    let mut available_sync_queues = HashSet::new();
    let mut needed_sync_endpoints = HashSet::new();
    let mut available_async_queues = HashSet::new();
    let mut needed_async_endpoints = HashSet::new();
    let mut sync_queues_size = 0;
    let mut async_queues_size = 0;
    let mut sync_queue_offsets = HashMap::new();
    let mut async_queue_offsets = HashMap::new();
    let mut messages_offsets = HashMap::new();
    let mut messages_size = 0;
    let mut sync_endpoints_size = 0;
    let mut sync_endpoints_offsets = HashMap::new();
    let mut async_endpoints_size = 0;
    let mut async_endpoints_offsets = HashMap::new();
    let mut sync_endpoints = HashMap::new();
    let mut async_endpoints = HashMap::new();
    let mut async_queues = HashMap::new();
    let message_len = config.async_message_len as usize;
    if message_len & 0x3 != 0 {
        return Err(
            PkgError::BadAsyncMessageLen {
                len: message_len
            }
        );
    }
    {
        let mut data = Vec::new();
        let filename = if args.debug {
            config.kernel.debug_src
        } else {
            config.kernel.release_src
        };
        let mut file = File::open(&filename).map_err(|_| 
            PkgError::ReadError {
                file: filename.to_string()
            }
        )?;
        file.read_to_end(&mut data).map_err(|_| 
            PkgError::ReadError {
                file: filename.to_string()
            }
        )?;
        file_data.insert("kernel".to_string(), (filename, data));
    }
    for program_config in config.programs {
        if Program::is_reserved_name(&program_config.name) {
            return Err(
                PkgError::ParseError {
                    file: args.config_file.to_string()
                }
            );
        }
        let mut regions = [const { Region::default() }; 8];
        let inter = if program_config.driver != 0 {
            if let Some(driver) = find_driver(program_config.driver) {
                regions[0] = Region { 
                    phys_addr: driver.base, 
                    virt_addr: driver.base | Region::ENABLE_MASK | ((RegionAttr::RW as u32) << Region::PERM_SHIFT) | Region::DEVICE_MASK, 
                    len: driver.len * 4
                };
                driver.inter
            } else {
                return Err(
                    PkgError::InvalidDriver {
                        name: program_config.name, 
                        driver: program_config.driver
                    }
                );
            }
        } else {
            0xff
        };

        let program = Program::new(
            program_config.name.to_string(),
            program_config.priority, 
            program_config.driver,
            inter,
            program_config.num_sync_queues,
            u32::try_from(program_config.sync_endpoints.len()).map_err(|_| 
                PkgError::ParseError {
                    file: program_config.name.to_string()
                }
            )?,
            u32::try_from(program_config.async_queues.len()).map_err(|_| 
                PkgError::ParseError {
                    file: program_config.name.to_string()
                }
            )?,
            u32::try_from(program_config.async_endpoints.len()).map_err(|_| 
                PkgError::ParseError {
                    file: program_config.name.to_string()
                }
            )?,
            regions 
        );

        if let Some(program) = programs.insert(program.name.to_string(), program) {
            return Err(
                PkgError::RepeatedProgram {
                    name: program.name
                }
            );
        }
        for i in 0..program_config.num_sync_queues {
            available_sync_queues.insert(Endpoint {
                name: program_config.name.to_string(),
                queue: i
            });
            sync_queue_offsets.insert(
                Endpoint {
                    name: program_config.name.to_string(),
                    queue: i
                },
                sync_queues_size
            );
            sync_queues_size += SYNC_QUEUE_SIZE;
        }
        for i in 0..program_config.async_queues.len() {
            available_async_queues.insert(Endpoint {
                name: program_config.name.to_string(),
                queue: i as u32
            });
            async_queue_offsets.insert(
                Endpoint {
                    name: program_config.name.to_string(),
                    queue: i as u32
                },
                async_queues_size
            );
            async_queues_size += ASYNC_QUEUE_SIZE;
            messages_offsets.insert(
                Endpoint {
                    name: program_config.name.to_string(),
                    queue: i as u32
                },
                messages_size
            );
            messages_size += program_config.async_queues[i] * (message_len + MESSAGE_HEADER_SIZE);
        }
        sync_endpoints_offsets.insert(program_config.name.to_string(), sync_endpoints_size);
        for endpoint in &program_config.sync_endpoints {
            needed_sync_endpoints.insert(endpoint.clone());
        }
        // endpoint is pointer to queue
        sync_endpoints_size += program_config.sync_endpoints.len() * ENDPOINT_SIZE;
        async_endpoints_offsets.insert(program_config.name.to_string(), async_endpoints_size);
        for endpoint in &program_config.async_endpoints {
            needed_async_endpoints.insert(endpoint.clone());
        }
        async_endpoints_size += program_config.async_endpoints.len() * ENDPOINT_SIZE;
        let mut data = Vec::new();
        let filename = if args.debug {
            program_config.debug_src
        } else {
            program_config.release_src
        };
        let mut file = File::open(&filename).map_err(|_| 
            PkgError::ReadError {
                file: filename.to_string()
            }
        )?;
        file.read_to_end(&mut data).map_err(|_| 
            PkgError::ReadError {
                file: filename.to_string()
            }
        )?;
        sync_endpoints.insert(program_config.name.to_string(), program_config.sync_endpoints);
        async_endpoints.insert(program_config.name.to_string(), program_config.async_endpoints);
        async_queues.insert(program_config.name.to_string(), program_config.async_queues);
        file_data.insert(program_config.name, (filename, data));
    }
    let sync_missing: Vec<_> = needed_sync_endpoints.difference(&available_sync_queues).map(|val| val.clone()).collect();
    if sync_missing.len() > 0 {
        return Err(
            PkgError::MissingSyncQueues {
                queues: sync_missing
            }
        );
    }
    let async_missing: Vec<_> = needed_async_endpoints.difference(&available_async_queues).map(|val| val.clone()).collect();
    if async_missing.len() > 0 {
        return Err(
            PkgError::MissingAsyncQueues {
                queues: async_missing
            }
        );
    }
    let mut files = HashMap::new();
    for (name, (filename, data)) in &file_data {
        let data = ElfFile::parse(data.as_ref()).map_err(|_| 
            PkgError::ReadError {
                file: filename.to_string()
            }
        )?;
        files.insert(name.to_string(), LoadedConfig {
            filename: filename.to_string(),
            data,
        });
    }
    let mut flash = MemMap::new("Flash", FLASH_START, FLASH_LEN);
    let mut ram = MemMap::new("RAM", RAM_START, RAM_LEN);
    let mut allocs = default_allocs(
        Program::get_prog_size() * programs.len(),
        PROC_SIZE * programs.len(),
        sync_queues_size,
        async_queues_size,
        sync_endpoints_size,
        async_endpoints_size,
        messages_size
    );
    for (name, file) in files {
        get_file_regions(&name, &file, &mut allocs, &mut renames)?;
    }

    let alloc_info = do_allocs(allocs, &mut ram, &mut flash, &mut sections, &mut programs)?;

    let username = whoami::username().unwrap();
    let path = format!("/tmp/pkg_{}", username);
    let path = Path::new(&path);
    if !path.exists() {
        fs::create_dir(path).map_err(|_| 
            PkgError::MkdirError {
                path: path.to_str().unwrap().to_string() 
            }
        )?;
    }
    let mut link_files = Vec::new();
    print_renames(&renames);
    for (file, (secs, symbols)) in renames {
        rename_file_sections(&args.objcopy, path, &file, &secs, &symbols, &mut link_files)?;
        // based on answer by embradded on https://stackoverflow.com/questions/68622938/new-versions-of-ld-cannot-take-elf-files-as-input-to-link accessed 11/02/2026
    }
    
    ram.display();
    flash.display();
    
    let mut sync_endpoints_vec: Vec<_> = sync_endpoints_offsets.iter().collect();
    sync_endpoints_vec.sort_by(|a, b| -> Ordering {
        if a.1 < b.1 {
            Ordering::Less
        } else if a.1 > b.1 {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    });
    let mut sync_endpoints_bytes = Vec::new();
    for (name, _) in sync_endpoints_vec {
        let endpoints = &sync_endpoints[name];
        for endpoint in endpoints {
            let queue_offset = sync_queue_offsets[&endpoint];
            let queue_addr = alloc_info.sync_queues_virt as u32 + queue_offset as u32;
            sync_endpoints_bytes.extend_from_slice(&queue_addr.to_le_bytes());
        }
    }
    let sync_endpoints_file = if sync_endpoints_bytes.len() > 0 {
        let sync_endpoints_file_bin = path.join("sync_endpoints.bin");
        let sync_endpoints_file = path.join("sync_endpoints.o").to_string_lossy().to_string().to_string();
        fs::write(&sync_endpoints_file_bin, sync_endpoints_bytes).map_err(|_| 
            PkgError::WriteError {
                file: sync_endpoints_file_bin.to_str().unwrap().to_string()
            }
        )?;
        let sync_endpoints_file_bin = sync_endpoints_file_bin.to_string_lossy().to_string().to_string();
        let mut cmd = Command::new(&args.objcopy);
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
                cmd: args.objcopy.to_string()
            }
        )?;
        Some(sync_endpoints_file)
    } else {
        None
    };

    let mut async_endpoints_vec: Vec<_> = async_endpoints_offsets.iter().collect();
    async_endpoints_vec.sort_by(|a, b| -> Ordering {
        if a.1 < b.1 {
            Ordering::Less
        } else if a.1 > b.1 {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    });
    let mut async_endpoints_bytes = Vec::new();
    for (name, _) in async_endpoints_vec {
        let endpoints = &async_endpoints[name];
        for endpoint in endpoints {
            let queue_offset = async_queue_offsets[&endpoint];
            let queue_addr = alloc_info.async_queues_virt as u32 + queue_offset as u32;
            async_endpoints_bytes.extend_from_slice(&queue_addr.to_le_bytes());
        }
    }
    let async_endpoints_file = if async_endpoints_bytes.len() > 0 {
        let async_endpoints_file_bin = path.join("async_endpoints.bin");
        let async_endpoints_file = path.join("async_endpoints.o").to_string_lossy().to_string().to_string();
        fs::write(&async_endpoints_file_bin, async_endpoints_bytes).map_err(|_| 
            PkgError::WriteError {
                file: async_endpoints_file_bin.to_str().unwrap().to_string()
            }
        )?;
        let async_endpoints_file_bin = async_endpoints_file_bin.to_string_lossy().to_string().to_string();
        let mut cmd = Command::new(&args.objcopy);
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
                cmd: args.objcopy.to_string()
            }
        )?;
        Some(async_endpoints_file)
    } else {
        None
    };

    let mut async_queues_vec: Vec<_> = async_queue_offsets.iter().collect();
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
        let messages_offset = messages_offsets[&name];
        let messages_addr = alloc_info.messages_virt + messages_offset;
        let buffer_len = async_queues[&name.name][name.queue as usize] as u32;
        let async_queue = AsyncQueue {
            buffer: messages_addr as u32,
            buffer_len,
            message_len: message_len as u32
        };
        async_queues_bytes.extend(async_queue.serialise());
    }
    let async_queues_file = if async_queues_bytes.len() > 0 {
        let async_queues_file_bin = path.join("async_queues.bin");
        let async_queues_file = path.join("async_queues.o").to_string_lossy().to_string().to_string();
        fs::write(&async_queues_file_bin, async_queues_bytes).map_err(|_| 
            PkgError::WriteError {
                file: async_queues_file_bin.to_str().unwrap().to_string()
            }
        )?;
        let async_queues_file_bin = async_queues_file_bin.to_string_lossy().to_string().to_string();
        let mut cmd = Command::new(&args.objcopy);
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
                cmd: args.objcopy.to_string()
            }
        )?;
        Some(async_queues_file)
    } else {
        None
    };


    let mut prog_table_bytes = Vec::new();
    prog_table_bytes.extend_from_slice(&(programs.len() as u32).to_ne_bytes());
    let mut programs = Vec::from_iter(programs.into_values());
    // plug in locations of queues and endpoints
    for program in &mut programs {
        if program.num_sync_endpoints > 0 {
            program.sync_endpoints = alloc_info.sync_endpoints_phys as u32 + sync_endpoints_offsets[&program.name] as u32;
        }
        if program.num_async_endpoints > 0 {
            program.async_endpoints = alloc_info.async_endpoints_phys as u32 + async_endpoints_offsets[&program.name] as u32;
        }
        let endpoint = Endpoint {
            name: program.name.to_string(),
            queue: 0,
        };
        if program.num_sync_queues > 0 {
            program.sync_queues = alloc_info.sync_queues_virt as u32 + sync_queue_offsets[&endpoint] as u32;
        }
        if program.num_async_queues > 0 {
            program.async_endpoints = alloc_info.async_queues_virt as u32 + async_queue_offsets[&endpoint] as u32;
        }
    }
    programs.sort_by(|p1, p2| -> Ordering {
        if p1.driver > p2.driver {
            Ordering::Greater
        } else if p1.driver == p2.driver {
            Ordering::Equal
        } else {
            Ordering::Less
        }
    });
    for program in &programs {
        prog_table_bytes.extend_from_slice(&program.serialise()?);
    }
    let prog_table_file_bin = path.join("prog_table.bin");
    let prog_table_file = path.join("prog_table.o").to_string_lossy().to_string().to_string();
    fs::write(&prog_table_file_bin, prog_table_bytes).map_err(|_| 
        PkgError::WriteError { 
            file: prog_table_file_bin.to_str().unwrap().to_string() 
        }
    )?;
    let prog_table_file_bin = prog_table_file_bin.to_string_lossy().to_string().to_string();

    let mut cmd = Command::new(&args.objcopy);
    cmd
        .arg("-O")
        .arg("elf32-littlearm")
        .arg("-I")
        .arg("binary")
        .arg("-B")
        .arg("arm")
        .arg(&prog_table_file_bin)
        .arg(&prog_table_file);
    check_cmd(cmd).map_err(|_| 
        PkgError::CmdError { 
            cmd: args.objcopy.to_string() 
        }
    )?;
    let link_file = create_link_file(
        &path, 
        &sections, 
        &alloc_info,
        &prog_table_file,
        async_queues_file.as_deref(),
        sync_endpoints_file.as_deref(),
        async_endpoints_file.as_deref()
    )?;
    let mut cmd = Command::new(&args.ld);
    for file in link_files {
        cmd.arg(file);
    }
    cmd
        .arg("-T")
        .arg(link_file)
        .arg("-e")
        .arg(&alloc_info.kernel_entry.to_string())
        .arg("-o")
        .arg(args.outfile)
        .arg("-z")
        .arg("noexecstack");
    check_cmd(cmd).map_err(|_| 
        PkgError::CmdError { 
            cmd: args.ld 
        }
    )?;
    for program in programs {
        println!("{:#x?}", program);
    }
    Ok(())
}

fn main() {
    let args: Vec<_> = env::args().collect();
    match run(args) {
        Ok(()) => {},
        Err(err) => {
            eprintln!("{}", Red.paint(err.to_string()));
            exit(1);
        }
    }
}
