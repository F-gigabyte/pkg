use std::{cmp::Ordering, collections::HashMap, env, fs::{self, File}, io::Read, path::Path, process::{Command, exit}};

use object::read::elf::ElfFile;

use ansi_term::Color::Red;

use crate::{allocs::{MemMap, default_allocs, do_allocs}, args::Args, cmds::check_cmd, drivers::find_driver, elf::get_file_regions, errors::PkgError, file_config::{Endpoint, FileConfig, LoadedConfig}, program::Program, queues::QueueRequirements, region::Region, region_attr::RegionAttr, sections::{Section, create_link_file, print_renames, rename_file_sections}};

pub mod drivers;
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

const PROC_SIZE: usize = 13 * 4;

const FLASH_START: usize = 0x10000000;
const FLASH_LEN: usize = 2048 * 1024;

const RAM_START: usize = 0x20000000;
const RAM_LEN: usize = 264 * 1024;

fn run(args: Vec<String>) -> Result<(), PkgError> {

    let args = Args::parse(&args)?;
    let config = FileConfig::parse(args.config_file)?;

    let mut sections = Vec::new();
    let mut renames = HashMap::new();
    let mut file_data = HashMap::new();
    let mut programs = HashMap::new();

    let message_len = config.async_message_len as usize;

    if message_len & 0x3 != 0 {
        return Err(
            PkgError::BadAsyncMessageLen {
                len: message_len
            }
        );
    }
    let mut queue_requirements = QueueRequirements::new(message_len);

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
    for mut program_config in config.programs {
        if Program::is_reserved_name(&program_config.name) {
            return Err(
                PkgError::ParseError {
                    file: args.config_file.to_string()
                }
            );
        }
        let mut regions = [const { Region::default() }; 8];
        let inter;
        if program_config.driver != 0 {
            let driver = find_driver(program_config.driver).ok_or(
                PkgError::InvalidDriver {
                        name: program_config.name.to_string(), 
                        driver: program_config.driver
                }
            )?; 
            regions[0] = Region { 
                phys_addr: driver.base, 
                virt_addr: driver.base | 
                    Region::ENABLE_MASK | 
                    ((RegionAttr::RW as u32) << Region::PERM_SHIFT) 
                    | Region::DEVICE_MASK, 
                len: driver.len * 4
            };
            inter = driver.inter;
        } else {
            inter = 0xff;
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

        queue_requirements.add_program_queues(&mut program_config);

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


        file_data.insert(program_config.name, (filename, data));
    }
    queue_requirements.requirements_satisfied()?;

    let queues = queue_requirements.get_queues();

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
        queues.sync_queues_size,
        queues.async_queues_size,
        queues.sync_endpoints_size,
        queues.async_endpoints_size,
        queues.messages_size
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
    
    ram.display(0);
    flash.display(0);
   
    let sync_endpoints_file = queues.write_sync_enpoints_file(path, &alloc_info, &args.objcopy)?;

    let async_endpoints_file = queues.write_async_endpoints_file(path, &alloc_info, &args.objcopy)?;

    let async_queues_file = queues.write_async_endpoints_file(path, &alloc_info, &args.objcopy)?;

    let mut prog_table_bytes = Vec::new();
    prog_table_bytes.extend_from_slice(&(programs.len() as u32).to_ne_bytes());
    let mut programs = Vec::from_iter(programs.into_values());
    // plug in locations of queues and endpoints
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
            program.async_endpoints = alloc_info.async_queues_virt as u32 + queues.async_queue_offsets[&endpoint] as u32;
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
        program.display(0);
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
