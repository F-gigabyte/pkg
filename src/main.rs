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

use std::{cmp::Ordering, collections::HashMap, env, fs::{self, File, read_dir}, io::Read, path::Path, process::{Command, exit}};

use object::read::elf::ElfFile;

use ansi_term::Color::Red;
use tempfile::TempDir;

use crate::{allocs::{MemMap, add_error_codes, default_allocs, do_allocs}, args::Args, cmds::check_cmd, driver_args::DriverArgs, drivers::{DriverError, PinError, find_driver, take_pins}, elf::{add_final_args_and_crcs, get_file_regions}, errors::PkgError, file_config::{Endpoint, FileConfig, LoadedConfig}, program::Program, queues::QueueRequirements, region::Region, region_attr::RegionAttr, sections::{Section, create_link_file, print_renames, rename_file_sections}};

pub mod devices;
pub mod queues;
pub mod section_attr;
pub mod region_attr;
pub mod region;
pub mod program;
pub mod errors;
pub mod allocs;
pub mod sections;
pub mod cmds;
pub mod file_config;
pub mod args;
pub mod elf;
pub mod driver_args;

const PROC_SIZE: usize = 12 * 4;

const FLASH_START: usize = 0x10000000;
const FLASH_LEN: usize = 2048 * 1024;

const RAM_START: usize = 0x20000000;
const RAM_LEN: usize = 264 * 1024;

/// Attempts to find a programs test file  
/// `dir` is the directory to the program's debug or release directory  
/// `name` is the program's executable name  
/// Returns the file's path on success or a `PkgError` on error
fn find_file(dir: &Path, name: &str) -> Result<String, PkgError> {
    let test_dir = dir.join("deps");
    let fingerprint_dir = dir.join(".fingerprint");
    // file name of format  name '-' 16 character hexadecimal hash value
    let dir = read_dir(test_dir).map_err(|_| {
        PkgError::NoFile { 
            file: name.to_string() 
        }
    })?;
    let mut res = Vec::new();
    let test_bin_name = format!("test-bin-{}", name);
    for file in dir {
        if let Ok(file) = file {
            let file_name = file.file_name().into_string().unwrap();
            let parts: Vec<_> = file_name.split('-').collect();
            if parts.len() == 2 {
                if parts[0] == name && parts[1].len() == 16 && parts[1].chars().all(|c| c.is_ascii_hexdigit()) {
                    let fingerprint = fingerprint_dir.join(&file_name);
                    if let Ok(fingerprint) = fingerprint.read_dir() {
                        for test_file in fingerprint {
                            if let Ok(test_file) = test_file {
                                if *test_file.file_name() == *test_bin_name {
                                    res.push(file.path().to_str().unwrap().to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    match res.len() {
        0 => Err(PkgError::NoFile { 
            file: name.to_string() 
        }),
        1 => Ok(res[0].to_string()),
        _ => Err(PkgError::MultipleFiles { 
            file: name.to_string(), 
            files: res 
        })
    }
}

/// Runs the application
fn run() -> Result<(), PkgError> {
    // parse arguments and config
    let args = Args::parse();
    let config = FileConfig::parse(&args.cmd_args.config_file)?;

    // load later data
    let mut sections = Vec::new();
    let mut renames = HashMap::new();
    let mut file_data = HashMap::new();
    let mut programs = HashMap::new();
    let mut driver_args = DriverArgs::new();

    let message_len = config.async_message_len as usize;

    // check message length
    if message_len & 0x3 != 0 {
        return Err(
            PkgError::BadAsyncMessageLen {
                len: message_len
            }
        );
    }
    let mut data = Vec::new();
    let mut queue_requirements = QueueRequirements::new(message_len);

    {
        // get kernel data
        // determine if this is a debug or release build
        let release = if let Some(release) = args.cmd_args.kernel_release {
            release
        } else {
            args.cmd_args.release
        };
        let test = if let Some(test) = args.cmd_args.kernel_test {
            test
        } else {
            args.cmd_args.test
        };
        let path = if release {
            Path::new(&config.release_path).to_path_buf()
        } else {
            Path::new(&config.debug_path).to_path_buf()
        };
        // locate required files
        let filename = if test {
            find_file(&path, &config.kernel.exec)?
        } else {
            path.join(config.kernel.exec).to_str().unwrap().to_string()
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
        file_data.insert("kernel".to_string(), (filename, 0, None, 0, data.len(), 0));
    }
    // get programs data
    let program_path = if args.cmd_args.release {
        Path::new(&config.release_path).to_path_buf()
    } else {
        Path::new(&config.debug_path).to_path_buf()
    };
    for mut program_config in config.programs {
        // check program name is valid
        if Program::is_reserved_name(&program_config.name) {
            return Err(
                PkgError::ParseError {
                    file: args.cmd_args.config_file.to_string()
                }
            );
        } else if program_config.block_len == 0 {
            return Err(
                PkgError::ParseError { 
                    file: args.cmd_args.config_file.to_string()
                }
            );
        }
        // get program regions and device information
        let mut regions = [const { Region::default() }; 8];
        let inter;
        let driver_num;
        let mut pin_mask = 0;
        if let Some(driver_name) = &program_config.driver {
            let driver = find_driver(driver_name).map_err(|err| {
                match err {
                    DriverError::Invalid => {
                        PkgError::InvalidDriver {
                            name: program_config.name.to_string(), 
                            driver: driver_name.to_string()
                        }
                    },
                    DriverError::Taken => {
                        PkgError::DriverTaken {
                            name: program_config.name.to_string(), 
                            driver: driver_name.to_string()
                        }
                    }
                }
            })?; 
            let empty_pins = vec![];
            let pins = program_config.pins.as_ref().unwrap_or(&empty_pins);
            take_pins(&mut driver_args, &pins, &driver).map_err(|err| {
                match err {
                    PinError::Taken(taken) => {
                        PkgError::PinsTaken { 
                            name: program_config.name.to_string(), 
                            driver: driver_name.to_string(), 
                            pins: taken 
                        }
                    },
                    PinError::Invalid(invalid) => {
                        PkgError::InvalidPins { 
                            name: program_config.name.to_string(), 
                            driver: driver_name.to_string(), 
                            pins: invalid 
                        }
                    }
                }
            })?;
            for pin in pins {
                pin_mask |= 1 << pin;
            }
            regions[0] = Region { 
                name: None,
                phys_addr: 0, 
                virt_addr: driver.base, 
                len: ((driver.len * 4).ilog2() - 1) << Region::LEN_SHIFT | 
                    Region::ENABLE_MASK | 
                    ((RegionAttr::RW as u32) << Region::PERM_SHIFT) 
                    | Region::DEVICE_MASK
                    | Region::VIRTUAL_MASK, 
                actual_len: driver.len * 4,
                codes: 0,
            };
            inter = driver.inter;
            driver_num = driver.num;
        } else {
            inter = [0xff; 4];
            driver_num = 0;
        };

        // check queue lengths are valid
        if program_config.num_sync_queues > 32 {
            return Err(PkgError::ParseError { 
                file: program_config.name.to_string()
            });
        }
        
        if program_config.num_notifiers > 32 {
            return Err(PkgError::ParseError { 
                file: program_config.name.to_string()
            });
        }

        let num_async_queues = program_config.async_queues.len();
        if num_async_queues > 32 {
            return Err(PkgError::ParseError { 
                file: program_config.name.to_string()
            });
        }
        
        // create program object for each program
        let program = Program::new(
            program_config.name.to_string(),
            program_config.priority, 
            driver_num,
            inter,
            program_config.num_sync_queues,
            u32::try_from(program_config.sync_endpoints.len()).map_err(|_| 
                PkgError::ParseError {
                    file: program_config.name.to_string()
                }
            )?,
            num_async_queues as u8,
            u32::try_from(program_config.async_endpoints.len()).map_err(|_| 
                PkgError::ParseError {
                    file: program_config.name.to_string()
                }
            )?,
            program_config.num_notifiers,
            regions,
            program_config.block_len,
            pin_mask,
            None
        );

        // check programs each have a unique name
        if let Some(program) = programs.insert(program.name.to_string(), program) {
            return Err(
                PkgError::RepeatedProgram {
                    name: program.name
                }
            );
        }

        // add program queues
        queue_requirements.add_program_queues(&mut program_config);

        let filename = if args.cmd_args.test {
            find_file(&program_path, &program_config.exec)?
        } else {
            program_path.join(program_config.exec).to_str().unwrap().to_string()
        };
        let mut file = File::open(&filename).map_err(|_| 
            PkgError::ReadError {
                file: filename.to_string()
            }
        )?;
        let start = data.len();
        file.read_to_end(&mut data).map_err(|_| 
            PkgError::ReadError {
                file: filename.to_string()
            }
        )?;

        file_data.insert(program_config.name, (filename, driver_num, program_config.pins, start, data.len(), program_config.block_len));
    }

    println!("Have pin functions {:#?}", driver_args.pin_func);
    println!("Have pin pads {:#x?}", driver_args.pads);

    // check queue requirements
    queue_requirements.requirements_satisfied()?;
    let queues = queue_requirements.get_queues();

    let mut files = HashMap::new();
    for (name, (filename, driver, pins, start, end, block_len)) in file_data {
        let data = ElfFile::parse(&data[start..end]).map_err(|_| 
            PkgError::ReadError {
                file: filename.to_string()
            }
        )?;
        files.insert(name.to_string(), LoadedConfig {
            filename: filename.to_string(),
            driver: driver,
            pins,
            data,
            block_len: block_len
        });
    }

    // allocate regions
    let mut flash = MemMap::new("Flash", FLASH_START, FLASH_LEN);
    let mut ram = MemMap::new("RAM", RAM_START, RAM_LEN);
    let mut allocs = default_allocs(
        // + 4 for the len bytes at the start
        Program::get_prog_size() * programs.len() + 4,
        PROC_SIZE * programs.len(),
        queues.sync_queues_size,
        queues.async_queues_size,
        queues.sync_endpoints_size,
        queues.async_endpoints_size,
        queues.messages_size,
        queues.notifier_size
    );
    let mut codes_offsets = HashMap::new();
    let mut codes_size = 0;
    for (name, file) in files {
        let s0 = get_file_regions(&name, &file, &mut allocs, &mut renames, &mut codes_offsets, codes_size)?;
        codes_size = s0;
    }

    // add error code region
    add_error_codes(&mut allocs, codes_size);

    let alloc_info = do_allocs(allocs, &mut ram, &mut flash, &mut sections, &mut programs)?;

    let root = TempDir::new().map_err(|_| PkgError::MkdirError)?;
    let mut link_files = Vec::new();
    print_renames(&renames);
    // rename file sections
    for (file, (secs, symbols)) in renames {
        rename_file_sections(&args.env_args.objcopy, &root, &file, &secs, &symbols, &mut link_files)?;
    }
    
    // display allocation memory map
    ram.display(0);
    flash.display(0);
   
    // write queue files
    let sync_endpoints_file = queues.write_sync_enpoints_file(&root, &alloc_info, &args.env_args.objcopy)?;

    let async_endpoints_file = queues.write_async_endpoints_file(&root, &alloc_info, &args.env_args.objcopy)?;

    let async_queues_file = queues.write_async_queues_file(&root, &alloc_info, &args.env_args.objcopy)?;

    let mut prog_table_bytes = Vec::new();
    prog_table_bytes.extend_from_slice(&(programs.len() as u32).to_le_bytes());
    let mut programs = Vec::from_iter(programs.into_values());
    // finalise program queue and endpoint locations
    // plug in locations of queues, endpoints and error codes
    for program in &mut programs {
        if program.num_sync_endpoints > 0 {
            program.sync_endpoints = alloc_info.sync_endpoints_phys as u32 + queues.sync_endpoints_offsets[&program.name] as u32;
        }
        if program.num_async_endpoints > 0 {
            program.async_endpoints = alloc_info.async_endpoints_phys as u32 + queues.async_endpoints_offsets[&program.name] as u32;
        }
        let endpoint = Endpoint {
            name: program.name.to_string(),
            queue: 0,
        };
        if program.num_sync_queues > 0 {
            program.sync_queues = alloc_info.sync_queues_virt as u32 + queues.sync_queue_offsets[&endpoint] as u32;
        }
        if program.num_async_queues > 0 {
            program.async_queues = alloc_info.async_queues_virt as u32 + queues.async_queue_offsets[&endpoint] as u32;
        }

        if program.num_notifiers > 0 {
            program.notifiers = alloc_info.notifier_virt as u32 + queues.notifier_offsets[&program.name] as u32;
        }

        for region in &mut program.regions {
            // regions that are enabled and not device need protection
            if region.is_enabled() && !region.is_device() && region.has_virt() {
                let name = region.name.as_ref().unwrap();
                let codes_name = format!("{}{}", program.name, name);
                if let Some(codes_offset) = codes_offsets.get(&codes_name) {
                    region.codes = (alloc_info.codes + *codes_offset) as u32;
                }
            }
        }
    }
    // sort programs by device
    programs.sort_by(|p1, p2| -> Ordering {
        if p1.driver > p2.driver {
            Ordering::Greater
        } else if p1.driver == p2.driver {
            Ordering::Equal
        } else {
            Ordering::Less
        }
    });
    // write program table to file
    for program in &programs {
        prog_table_bytes.extend_from_slice(&program.serialise()?);
    }
    let prog_table_file_bin = root.path().join("prog_table.bin");
    let prog_table_file = root.path().join("prog_table.o").to_string_lossy().to_string().to_string();
    fs::write(&prog_table_file_bin, prog_table_bytes).map_err(|_| 
        PkgError::WriteError { 
            file: prog_table_file_bin.to_str().unwrap().to_string() 
        }
    )?;
    let prog_table_file_bin = prog_table_file_bin.to_string_lossy().to_string().to_string();
    
    let mut cmd = Command::new(&args.env_args.objcopy);
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
            cmd: args.env_args.objcopy.to_string() 
        }
    )?;

    // write kernel driver args to file
    let args_file_bin = root.path().join("args.bin");
    let args_file = root.path().join("args.o").to_string_lossy().to_string().to_string();
    fs::write(&args_file_bin, driver_args.serialise()).map_err(|_|
        PkgError::WriteError { 
            file: args_file_bin.to_str().unwrap().to_string() 
        }
    )?;
    let args_file_bin = args_file_bin.to_string_lossy().to_string().to_string();
    let mut cmd = Command::new(&args.env_args.objcopy);
    cmd
        .arg("-O")
        .arg("elf32-littlearm")
        .arg("-I")
        .arg("binary")
        .arg("-B")
        .arg("arm")
        .arg(&args_file_bin)
        .arg(&args_file);
    check_cmd(cmd).map_err(|_| 
        PkgError::CmdError { 
            cmd: args.env_args.objcopy.to_string() 
        }
    )?;


    // create the final link file
    let link_file = create_link_file(
        &root, 
        &sections, 
        &alloc_info,
        &prog_table_file,
        async_queues_file.as_deref(),
        sync_endpoints_file.as_deref(),
        async_endpoints_file.as_deref(),
        &args_file
    )?;
    let mut cmd = Command::new(&args.env_args.ld);
    for file in link_files {
        cmd.arg(file);
    }
    let exe_file = root.path().join("small_os.o");
    cmd
        .arg("-T")
        .arg(link_file)
        .arg("-e")
        .arg(&alloc_info.kernel_entry.to_string())
        .arg("-o")
        .arg(&exe_file)
        .arg("-z")
        .arg("noexecstack");
    check_cmd(cmd).map_err(|_| 
        PkgError::CmdError { 
            cmd: args.env_args.ld 
        }
    )?;
    
    // patch in stack argument locations and CRCs
    add_final_args_and_crcs(&programs, exe_file.as_os_str().to_str().unwrap(), &args.cmd_args.outfile)?;

    // display the program information
    for program in programs {
        program.display(0);
    }
    Ok(())
}

/// Main function
fn main() {
    // attempt to run and print error on failure
    match run() {
        Ok(()) => {},
        Err(err) => {
            eprintln!("{}", Red.paint(err.to_string()));
            exit(1);
        }
    }
}
