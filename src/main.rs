use std::{cmp::Ordering, collections::{HashMap, HashSet}, env, fs::{self, File}, io::Read, path::Path, process::{Command, exit}};
use serde::Deserialize;

use object::{Endianness, Object, ObjectKind, ObjectSection, ObjectSymbol, SectionIndex, StringTable, elf::{SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, STT_FUNC}, read::elf::{ElfFile, ElfFile32, FileHeader, SectionHeader}};

use crate::{async_queue::AsyncQueue, drivers::find_driver, errors::PkgError, program::Program, region::Region, region_attr::RegionAttr, section_attr::SectionAttr};

pub mod drivers;
pub mod async_queue;
pub mod section_attr;
pub mod region_attr;
pub mod endpoints;
pub mod region;
pub mod program;
pub mod errors;

const MIN_SIZE: u32 = 256;

// length of sync queue in bytes
const SYNC_QUEUE_LEN: usize = 16;

// length of async queue in bytes
const ASYNC_QUEUE_LEN: usize = 28;

// message header length in bytes
const MESSAGE_HEADER_LEN: usize = 12;

const ENDPOINT_LEN: usize = 4;

const BOOTLOADER_ADDR: usize = 0x10000000;
const VECTORS_ADDR: usize = 0x10000000 + 256;

const PROC_LEN: usize = 13 * 4;

#[derive(Debug)]
pub struct Block {
    lower: usize,
    upper: usize,
    region: String
}

#[derive(Debug, Clone)]
pub struct Alloc {
    name: String,
    region: String,
    queue: bool,
    need_region: bool,
    attr: RegionAttr,
    load: bool,
    store: bool,
    entry_addr: Option<usize>,
    size: usize,
    alignment: usize
}

pub struct SectionRename {
    old_name: String,
    new_name: String,
}

pub struct Section {
    name: String,
    phys_addr: usize,
    virt_addr: usize,
}

pub struct AllocInfo {
    kernel_entry: usize,
    kernel_stack: usize,
    prog_table_phys: usize,
    sync_queues_virt: usize,
    sync_queues_len: usize,
    async_queues_phys: usize,
    async_queues_virt: usize,
    async_queues_len: usize,
    messages_virt: usize,
    messages_len: usize,
    sync_endpoints_phys: usize,
    async_endpoints_phys: usize,
    proc_virt: usize,
    proc_len: usize
}

fn do_allocs(allocs: Vec<Alloc>, ram: &mut Vec<Block>, flash: &mut Vec<Block>, sections: &mut Vec<Section>, programs: &mut HashMap<String, Program>) -> Result<AllocInfo, PkgError> {
    let mut kernel_entry = None;
    let mut kernel_stack = None;
    let mut prog_table_phys = None;
    let mut sync_queues_virt = None;
    let mut sync_queues_len = None;
    let mut async_queues_phys = None;
    let mut async_queues_virt = None;
    let mut async_queues_len = None;
    let mut messages_virt = None;
    let mut messages_len = None;
    let mut sync_endpoints_phys = None;
    let mut async_endpoints_phys = None;
    let mut procs_virt = None;
    let mut procs_len = None;
    for mut alloc in allocs {
        let is_kernel = if alloc.name == "kernel" { 
            true
        } else {
            false
        };
        let is_sync = if alloc.name == "sync" {
            true
        } else {
            false
        };
        let is_async = if alloc.name == "async" {
            true
        } else {
            false
        };
        let is_prog_table = if alloc.name == "program_table" {
            true
        } else {
            false
        };
        let is_procs = if alloc.name == "procs" {
            true
        } else {
            false
        };
        let is_queue = alloc.queue;
        let need_region = alloc.need_region;
        let mut virt_addr = None;
        let attr = alloc.attr;
        let len = alloc.size;
        let alloc_name = alloc.name.clone();
        let region = alloc.region.clone();
        let name = format!("{}{}", alloc.name, alloc.region);
        let entry_addr = alloc.entry_addr.take();
        if alloc.load {
            virt_addr = Some(
                try_alloc_with_alloc(alloc.clone(), ram).map_err(|_| 
                    PkgError::NoSpace { 
                        name: alloc_name.clone(), 
                        region: region.clone()
                    }
                )?
            );
        }
        let mut phys_addr = None;
        if alloc.store {
            if is_kernel && alloc.region == ".bootloader" {
                try_alloc(
                    Block { 
                        lower: BOOTLOADER_ADDR, 
                        upper: BOOTLOADER_ADDR + alloc.size, 
                        region: format!("{}{} ({})", alloc_name.clone(), region.clone(), alloc.attr)
                    },
                    flash
                ).map_err(|_| 
                    PkgError::NoSpace { 
                        name: alloc_name.clone(), 
                        region: region.clone() 
                    }
                )?;
                phys_addr = Some(BOOTLOADER_ADDR);
            } else if is_kernel && alloc.region == ".text.vectors" {
                try_alloc(
                    Block { 
                        lower: VECTORS_ADDR, 
                        upper: VECTORS_ADDR + alloc.size, 
                        region: format!("{}{} ({})", alloc_name.clone(), region.clone(), alloc.attr)
                    },
                    flash
                ).map_err(|_| 
                    PkgError::NoSpace { 
                        name: alloc_name.clone(), 
                        region: region.clone() 
                    }
                )?;
                phys_addr = Some(VECTORS_ADDR);
            } else {
                if alloc.load && !is_kernel {
                    // if loading section, don't need to align physical address to size boundary
                    alloc.alignment = 4;
                }
                phys_addr = Some(try_alloc_with_alloc(alloc, flash).map_err(|_| 
                        PkgError::NoSpace {
                            name: alloc_name.clone(), 
                            region: region.clone() 
                        }
                    )?
                );
            }
        }
        if virt_addr.is_none() && phys_addr.is_none() {
            continue;
        }
        let virt_addr = virt_addr.unwrap_or_else(|| phys_addr.unwrap());
        let phys_addr = phys_addr.unwrap_or(virt_addr);
        if region != ".stack" && !is_queue && !is_prog_table {
            let alloc_sec = Section {
                name,
                phys_addr,
                virt_addr,
            };
            sections.push(alloc_sec);
        }
        if need_region {
            if let Some(program) = programs.get_mut(&alloc_name) {
                let sec = program.find_empty_region().ok_or(
                    PkgError::TooManySections {
                        name: alloc_name.to_string()
                    }
                )?;
                sec.len = len as u32;
                sec.phys_addr = phys_addr as u32;
                let zero = if region == ".bss" {
                    Region::ZERO_MASK
                } else {
                    0
                };
                sec.virt_addr = virt_addr as u32 | ((attr as u32) << Region::PERM_SHIFT) | Region::ENABLE_MASK | zero;
                if region == ".stack" {
                    program.sp = Some(virt_addr as u32 + len as u32);
                }
                if let Some(entry_addr) = entry_addr {
                    program.entry = Some(entry_addr as u32 + virt_addr as u32);
                }
            } else {
                return Err(
                    PkgError::NoProgram {
                        name: alloc_name.to_string()
                    }
                );
            }
        } else if is_kernel {
            if let Some(entry_addr) = entry_addr {
                kernel_entry = Some(entry_addr + virt_addr);
            }
            if region == ".stack" {
                kernel_stack = Some(virt_addr + len);
            }
        } else if is_prog_table {
            prog_table_phys = Some(phys_addr)
        } else if is_sync {
            match region.as_ref() {
                ".queues" => {
                    sync_queues_virt = Some(virt_addr);
                    sync_queues_len = Some(len);
                }
                ".endpoints" => sync_endpoints_phys = Some(phys_addr),
                _ => {}
            }
        } else if is_async {
            match region.as_ref() {
                ".queues" => {
                    async_queues_virt = Some(virt_addr);
                    async_queues_phys = Some(phys_addr);
                    async_queues_len = Some(len);
                },
                ".endpoints" => async_endpoints_phys = Some(phys_addr),
                ".messages" => {
                    messages_virt = Some(virt_addr);
                    messages_len = Some(len);
                }
                _ => {}
            }
        } else if is_procs {
            procs_virt = Some(virt_addr);
            procs_len = Some(len);
        }
    }
    Ok(AllocInfo { 
        kernel_entry: kernel_entry.ok_or(PkgError::NoKernelEntry)?, 
        kernel_stack: kernel_stack.ok_or(PkgError::NoKernelStack)?, 
        prog_table_phys: prog_table_phys.unwrap(), 
        sync_queues_virt: sync_queues_virt.unwrap(), 
        sync_queues_len: sync_queues_len.unwrap(),
        async_queues_phys: async_queues_phys.unwrap(), 
        async_queues_virt: async_queues_virt.unwrap(), 
        async_queues_len: async_queues_len.unwrap(), 
        messages_virt: messages_virt.unwrap(), 
        messages_len: messages_len.unwrap(),
        sync_endpoints_phys: sync_endpoints_phys.unwrap(),
        async_endpoints_phys: async_endpoints_phys.unwrap(),
        proc_virt: procs_virt.unwrap(),
        proc_len: procs_len.unwrap()
    })
}

fn get_file_regions(name: &str, file: &LoadedConfig, allocs: &mut Vec<Alloc>, renames: &mut HashMap<String, (Vec<SectionRename>, Vec<String>)>) -> Result<(), PkgError> {
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
            allocs.push(Alloc {
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
            });
        }
    }



    if let Some(stack) = file.data.symbol_by_name("__stack_size") {
        // if have reserved a stack, allocate this as well
        allocs.push(Alloc { 
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
        });
    }
    // add section renames to hash map
    _ = renames.insert(file.filename.to_string(), (file_secs, file_symbols));
    Ok(())
}

fn check_cmd(mut cmd: Command) -> Result<(), i32> {
    let status = cmd.output().map_err(|_| 0)?.status.code().ok_or(0)?;
    if  status == 0 {
        Ok(())
    } else {
        Err(status)
    }
}

fn try_alloc(block: Block, region: &mut Vec<Block>) -> Result<(), Block> {
    for i in 0..region.len() {
        if region[i].lower <= block.lower && region[i].upper >= block.upper && region[i].region == "free" {
            let mut i = i;
            if block.lower - region[i].lower != 0 {
                region.insert(i, Block { lower: region[i].lower, upper: block.lower, region: "free".to_string() });
                i += 1;
            }
            if region[i].upper - block.upper != 0 {
                region.insert(i + 1, Block { lower: block.upper, upper: region[i].upper, region: "free".to_string() });
            }
            region[i] = block;
            return Ok(());
        }
    }
    Err(block)
}

fn try_alloc_with_alloc(alloc: Alloc, region: &mut Vec<Block>) -> Result<usize, Alloc> {
    println!("Alloc is {:x?}", alloc);
    let mut suggest = None;
    let mut overflow = 0;
    for i in 0..region.len() {
        if region[i].region == "free" {
            let lower = (region[i].lower + alloc.alignment - 1) & !(alloc.alignment - 1);
            if lower < region[i].upper {
                let size = region[i].upper - lower;
                if size >= alloc.size {
                    let region_overflow = size - alloc.size;
                    if let Some(_) = suggest {
                        if region_overflow < overflow {
                            suggest = Some(i);
                            overflow = region_overflow;
                        }
                    } else {
                        suggest = Some(i);
                        overflow = region_overflow;
                    }
                }
            }
        }
    }
    if let Some(mut suggest) = suggest {
        let lower = (region[suggest].lower + alloc.alignment - 1) & !(alloc.alignment - 1);
        let upper = lower + alloc.size;
        if lower != region[suggest].lower {
            region.insert(suggest, Block {
                lower: region[suggest].lower,
                upper: lower,
                region: "free".to_string()
            });
            suggest += 1;
        }
        if upper != region[suggest].upper {
            region.insert(suggest + 1, Block {
                lower: upper,
                upper: region[suggest].upper,
                region: "free".to_string()
            });
        }
        region[suggest] = Block {
            lower,
            upper,
            region: format!("{}{} ({})", alloc.name, alloc.region, alloc.attr)
        };
        Ok(lower)
    } else {
        Err(alloc)
    }
}

fn rename_file_sections(objcopy: &str, path: &Path, file: &str, sections: &Vec<SectionRename>, symbols: &Vec<String>, link_files: &mut Vec<String>) -> Result<String, PkgError> {
    let file_name = Path::new(&file).file_name().unwrap().to_string_lossy().to_string();
    let res_name = path.join(&file_name).to_string_lossy().to_string().to_string();
    link_files.push(res_name.clone());
    let mut cmd = Command::new(&objcopy);
    for sec in sections {
        cmd.arg("--rename-section").arg(&format!("{}={}", sec.old_name, sec.new_name));
    }
    for symbol in symbols {
        cmd.arg(&format!("--localize-symbol={}", symbol));
    }
    cmd.arg(&file).arg(&res_name);
    check_cmd(cmd).map_err(|_| 
        PkgError::CmdError {
            cmd: objcopy.to_string() 
        }
    )?; 
    Ok(res_name)
}

fn create_link_file(
    path: &Path, 
    sections: &Vec<Section>, 
    alloc_info: &AllocInfo,
    prog_table_file: &str, 
    async_queues_file: Option<&str>,
    sync_endpoints_file: Option<&str>,
    async_endpoints_file: Option<&str>
    ) -> Result<String, PkgError> {

    let mut link_data = String::new();
    let mut have_bss = false;
    link_data.push_str("SECTIONS {");
    for sec in sections {
        if sec.name == "kernel.bss" {
            have_bss = true;
        }
        let symbol_name = sec.name.replace(".", "_");
        link_data = format!(
            "{}
            \t{} 0x{:x} : AT(0x{:x}) {{
            \t\t*({});
            \t}}
            \t__{}_phys_start = LOADADDR({});
            \t__{}_phys_end = LOADADDR({}) + SIZEOF({});
            \t__{}_virt_start = ADDR({});
            \t__{}_virt_end = ADDR({}) + SIZEOF({});", 
            link_data, 
            sec.name, 
            sec.virt_addr, 
            sec.phys_addr, 
            sec.name, 
            symbol_name, 
            sec.name,
            symbol_name,
            sec.name,
            sec.name,
            symbol_name, 
            sec.name,
            symbol_name,
            sec.name,
            sec.name
        );
    }
    if !have_bss {
        link_data = format!(
            "{}
            \t__kernel_bss_phys_start = 0;
            \t__kernel_bss_phys_end = 0;
            \t__kernel_bss_virt_start = 0;
            \t__kernel_bss_virt_end = 0;", 
            link_data
        );
    }
    link_data = format!("{}\n\tprogram_table 0x{:x} : AT(0x{:x}) {{\n\t\t__program_table = .;\n\t\t{}\n\t}}", link_data, alloc_info.prog_table_phys, alloc_info.prog_table_phys, prog_table_file);
    if let Some(sync_endpoints_file) = sync_endpoints_file {
        link_data = format!("{}\n\tsync_endpoints 0x{:x} : AT(0x{:x}) {{\n\t\t{}\n\t}}", link_data, alloc_info.sync_endpoints_phys, alloc_info.sync_endpoints_phys, sync_endpoints_file);
    }
    if let Some(async_endpoints_file) = async_endpoints_file {
        link_data = format!("{}\n\tasync_endpoints 0x{:x} : AT(0x{:x}) {{\n\t\t{}\n\t}}", link_data, alloc_info.async_endpoints_phys, alloc_info.async_endpoints_phys, async_endpoints_file);
    }
    if let Some(async_queues_file) = async_queues_file {
        link_data = format!(
            "{}
            \tasync_queues 0x{:x} : AT(0x{:x}) {{
            \t\t{}
            \t}}, 
            \t__async_queues_phys_start = LOADADDR(async_queues);
            \t__async_queues_phys_end = LOADADDR(async_queues) + SIZEOF(async_queues);
            \t__async_queues_virt_start = ADDR(async_queues);
            \t__async_queues_virt_end = ADDR(async_queues) + SIZEOF(async_queues);", 
            link_data, 
            alloc_info.async_queues_phys, 
            alloc_info.async_queues_virt, 
            async_queues_file
        );
    } else {
        link_data = format!(
            "{}
            \t__async_queues_phys_start = 0x{:x};
            \t__async_queues_phys_end = 0x{:x};
            \t__async_queues_virt_start = 0x{:x};
            \t__async_queues_virt_end = 0x{:x};", 
            link_data, 
            alloc_info.async_queues_phys, 
            alloc_info.async_queues_phys, 
            alloc_info.async_queues_virt, 
            alloc_info.async_queues_virt, 
        );
    }
    link_data = format!(
        "{}
        \t__sync_queues_virt_start = 0x{:x};
        \t__sync_queues_virt_end = 0x{:x} + 0x{:x};", 
        link_data, 
        alloc_info.sync_queues_virt, 
        alloc_info.sync_queues_virt, 
        alloc_info.sync_queues_len
    );
    link_data = format!(
        "{}
        \t__procs_virt_start = 0x{:x};
        \t__procs_virt_end = 0x{:x} + 0x{:x};", 
        link_data, 
        alloc_info.proc_virt, 
        alloc_info.proc_virt, 
        alloc_info.proc_len
    );
    link_data = format!("{}\n\t__kernel_stack = 0x{:x};", link_data, alloc_info.kernel_stack);
    link_data.push_str("\n}\n");
    let link_file = path.join("link.ld");
    fs::write(&link_file, link_data).map_err(|_| 
        PkgError::WriteError {
            file: "link.ld".to_string()
        }
    )?;
    let link_file = link_file.to_string_lossy().to_string().to_string();
    Ok(link_file)
}

fn print_mem_map(region: &Vec<Block>) {
    for block in region {
        println!("0x{:x} -> 0x{:x}: {}", block.lower, block.upper, block.region);
    }
}

fn print_renames(renames: &HashMap<String, (Vec<SectionRename>, Vec<String>)>) {
    for (file, (secs, _)) in renames.iter() {
        println!("{}", file);
        for sec in secs {
            println!("\t{} -> {}", sec.old_name, sec.new_name);
        }
    }
}

#[derive(Deserialize)]
struct KernelConfig {
    debug_src: String,
    release_src: String,
}

#[derive(Debug, Deserialize, Hash, PartialEq, Eq, Clone)]
pub struct Endpoint {
    name: String,
    queue: u32
}

#[derive(Deserialize)]
struct ProgramConfig {
    name: String,
    priority: u8,
    driver: u16,
    debug_src: String,
    release_src: String,
    num_sync_queues: u32,
    async_queues: Vec<usize>,
    sync_endpoints: Vec<Endpoint>,
    async_endpoints: Vec<Endpoint>
}

#[derive(Deserialize)]
struct FileConfig {
    async_message_len: u32,
    kernel: KernelConfig,
    programs: Vec<ProgramConfig>
}

struct LoadedConfig<'a> {
    filename: String,
    data: ElfFile32<'a>,
}

fn run(args: Vec<String>) -> Result<(), PkgError> {
    let objcopy = env::var("OBJCOPY").unwrap_or("objcopy".to_string());
    let ld = env::var("LD").unwrap_or("ld".to_string());
    let mut config_index = 1;
    let mut sections = Vec::new();
    if args.len() < 4 || args.len() > 5 {
        return Err(
            PkgError::InvalidArgs {
                name: args[0].clone()
            }
        );
    }
    let debug = if args.len() == 5 {
        match args[1].as_ref() {
            "-r" => {
                config_index = 2;
                false
            },
            _ => {
                return Err(
                    PkgError::InvalidArgs {
                        name: args[0].clone()
                    }
                );
            }
        }
    } else {
        true
    };
    if args[config_index + 1] != "-o" {
        return Err(
            PkgError::InvalidArgs {
                name: args[0].clone()
            }
        );
    }
    let outfile = &args[config_index + 2];
    let config_file = &args[config_index];
    let config = fs::read_to_string(config_file).map_err(|_| 
        PkgError::ReadError {
            file: config_file.to_string() 
        }
    )?;
    let config = toml::from_str::<FileConfig>(&config).map_err(|_| 
        PkgError::ParseError {
            file: config_file.to_string()
        }
    )?;
    let mut renames = HashMap::new();
    let mut file_data = HashMap::new();
    let mut programs = HashMap::new();
    let mut available_sync_queues = HashSet::new();
    let mut needed_sync_endpoints = HashSet::new();
    let mut available_async_queues = HashSet::new();
    let mut needed_async_endpoints = HashSet::new();
    let mut sync_queues_len = 0;
    let mut async_queues_len = 0;
    let mut sync_queue_offsets = HashMap::new();
    let mut async_queue_offsets = HashMap::new();
    let mut messages_offsets = HashMap::new();
    let mut messages_len = 0;
    let mut sync_endpoints_len = 0;
    let mut sync_endpoints_offsets = HashMap::new();
    let mut async_endpoints_len = 0;
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
        let filename = if debug {
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
    for program in config.programs {
        if program.name == "kernel" || program.name == "sync" || program.name == "async" || program.name == "program_table" || program.name == "procs" {
            return Err(
                PkgError::ParseError {
                    file: config_file.to_string()
                }
            );
        }
        let mut regions = [const { Region::default() }; 8];
        let inter = if program.driver != 0 {
            if let Some(driver) = find_driver(program.driver) {
                regions[0] = Region { 
                    phys_addr: driver.base, 
                    virt_addr: driver.base | Region::ENABLE_MASK | ((RegionAttr::RW as u32) << Region::PERM_SHIFT) | Region::DEVICE_MASK, 
                    len: driver.len * 4
                };
                driver.inter
            } else {
                return Err(
                    PkgError::InvalidDriver {
                        name: program.name.to_string(), 
                        driver: program.driver
                    }
                );
            }
        } else {
            0xff
        };
        if let Some(program) = programs.insert(
            program.name.to_string(), 
            Program { 
                name: program.name.to_string(),
                priority: program.priority, 
                driver: program.driver,
                inter,
                sp: None, 
                entry: None, 
                num_sync_queues: program.num_sync_queues,
                num_sync_endpoints: u32::try_from(program.sync_endpoints.len()).map_err(|_| 
                    PkgError::ParseError {
                        file: program.name.to_string()
                    }
                )?,
                sync_queues: 0,
                sync_endpoints: 0,
                num_async_queues: u32::try_from(program.async_queues.len()).map_err(|_| 
                    PkgError::ParseError {
                        file: program.name.to_string()
                    }
                )?,
                num_async_endpoints: u32::try_from(program.async_endpoints.len()).map_err(|_| 
                    PkgError::ParseError {
                        file: program.name.to_string()
                    }
                )?,
                async_queues: 0,
                async_endpoints: 0,
                regions 
            }
        ) {
            return Err(
                PkgError::RepeatedProgram {
                    name: program.name
                }
            );
        }
        for i in 0..program.num_sync_queues {
            available_sync_queues.insert(Endpoint {
                name: program.name.to_string(),
                queue: i
            });
            sync_queue_offsets.insert(
                Endpoint {
                    name: program.name.to_string(),
                    queue: i
                },
                sync_queues_len
            );
            sync_queues_len += SYNC_QUEUE_LEN;
        }
        for i in 0..program.async_queues.len() {
            available_async_queues.insert(Endpoint {
                name: program.name.to_string(),
                queue: i as u32
            });
            async_queue_offsets.insert(
                Endpoint {
                    name: program.name.to_string(),
                    queue: i as u32
                },
                async_queues_len
            );
            async_queues_len += ASYNC_QUEUE_LEN;
            messages_offsets.insert(
                Endpoint {
                    name: program.name.to_string(),
                    queue: i as u32
                },
                messages_len
            );
            messages_len += program.async_queues[i] * (message_len + MESSAGE_HEADER_LEN);
        }
        sync_endpoints_offsets.insert(program.name.to_string(), sync_endpoints_len);
        for endpoint in &program.sync_endpoints {
            needed_sync_endpoints.insert(endpoint.clone());
        }
        // endpoint is pointer to queue
        sync_endpoints_len += program.sync_endpoints.len() * ENDPOINT_LEN;
        async_endpoints_offsets.insert(program.name.to_string(), async_endpoints_len);
        for endpoint in &program.async_endpoints {
            needed_async_endpoints.insert(endpoint.clone());
        }
        async_endpoints_len += program.async_endpoints.len() * ENDPOINT_LEN;
        let mut data = Vec::new();
        let filename = if debug {
            program.debug_src
        } else {
            program.release_src
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
        sync_endpoints.insert(program.name.to_string(), program.sync_endpoints);
        async_endpoints.insert(program.name.to_string(), program.async_endpoints);
        async_queues.insert(program.name.to_string(), program.async_queues);
        file_data.insert(program.name, (filename, data));
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
    let mut flash = Vec::new();
    flash.push(Block {
        lower: 0x10000000,
        upper: 0x10000000 + 2048 * 1024,
        region: "free".to_string()
    });
    let mut ram = Vec::new();
    ram.push(Block {
        lower: 0x20000000,
        upper: 0x20000000 + 264 * 1024,
        region: "free".to_string()
    });
    let mut allocs = Vec::new();
    for (name, file) in files {
        get_file_regions(&name, &file, &mut allocs, &mut renames)?;
    }
    let prog_table_size = Program::get_prog_size() * programs.len();
    let prog_table_alloc = Alloc {
        name: "program_table".to_string(),
        region: ".program_table".to_string(),
        queue: false,
        need_region: false,
        attr: RegionAttr::R,
        load: false,
        store: true,
        entry_addr: None,
        size: prog_table_size,
        alignment: 4
    };
    allocs.push(prog_table_alloc);
    let procs_len = PROC_LEN * programs.len();
    let procs_alloc = Alloc {
        name: "procs".to_string(),
        region: ".procs".to_string(),
        queue: true,
        need_region: false,
        attr: RegionAttr::RW,
        load: true,
        store: false,
        entry_addr: None,
        size: procs_len,
        alignment: 4
    };
    allocs.push(procs_alloc);
    let sync_queues_alloc = Alloc {
        name: "sync".to_string(),
        region: ".queues".to_string(),
        queue: true,
        need_region: false,
        attr: RegionAttr::RW,
        load: true,
        store: false,
        entry_addr: None,
        size: sync_queues_len,
        alignment: 4
    };
    allocs.push(sync_queues_alloc);
    let async_queues_alloc = Alloc {
        name: "async".to_string(),
        region: ".queues".to_string(),
        queue: true,
        need_region: false,
        attr: RegionAttr::RW,
        load: true,
        store: true,
        entry_addr: None,
        size: async_queues_len,
        alignment: 4
    };
    allocs.push(async_queues_alloc);
    let endpoints_alloc = Alloc {
        name: "sync".to_string(),
        region: ".endpoints".to_string(),
        queue: true,
        need_region: false,
        attr: RegionAttr::R,
        load: false,
        store: true,
        entry_addr: None,
        size: sync_endpoints_len,
        alignment: 4
    };
    allocs.push(endpoints_alloc);
    let async_endpoints_alloc = Alloc {
        name: "async".to_string(),
        region: ".endpoints".to_string(),
        queue: true,
        need_region: false,
        attr: RegionAttr::R,
        load: false,
        store: true,
        entry_addr: None,
        size: async_endpoints_len,
        alignment: 4
    };
    allocs.push(async_endpoints_alloc);
    let messages_alloc = Alloc {
        name: "async".to_string(),
        region: ".messages".to_string(),
        queue: false,
        need_region: false,
        attr: RegionAttr::RW,
        load: true,
        store: false,
        entry_addr: None,
        size: messages_len,
        alignment: 4
    };
    allocs.push(messages_alloc);
    // sort allocs from smallest alignment to largest alignment
    allocs.sort_by(|a, b| 
        match (a.name == "kernel", b.name == "kernel") {
            (true, true) => {
                if a.region == ".bootloader" {
                    Ordering::Less
                } else if b.region == ".bootloader" {
                    Ordering::Greater
                } else if a.region == ".text.vectors" {
                    Ordering::Less
                } else if b.region == ".text.vectors" {
                    Ordering::Greater
                } else if a.alignment == b.alignment { 
                    if a.size == b.size {
                        Ordering::Equal
                    } else if a.size < b.size {
                        Ordering::Less
                    } else {
                        Ordering::Greater 
                    }
                } else if a.alignment < b.alignment { 
                    Ordering::Less 
                } else { 
                    Ordering::Greater 
                }
            },
            (true, false) => {
                Ordering::Less
            },
            (false, true) => {
                Ordering::Greater
            },
            (_, _) => {
                if a.alignment == b.alignment { 
                    if a.size == b.size {
                        Ordering::Equal
                    } else if a.size < b.size {
                        Ordering::Less
                    } else {
                        Ordering::Greater 
                    }
                } else if a.alignment < b.alignment { 
                    Ordering::Less 
                } else { 
                    Ordering::Greater 
                }
            }
        }
    );
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
        rename_file_sections(&objcopy, path, &file, &secs, &symbols, &mut link_files)?;
        // based on answer by embradded on https://stackoverflow.com/questions/68622938/new-versions-of-ld-cannot-take-elf-files-as-input-to-link accessed 11/02/2026
    }
    
    println!("RAM");
    print_mem_map(&ram);
    println!("Flash");
    print_mem_map(&flash);
    
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
        let mut cmd = Command::new(&objcopy);
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
        let mut cmd = Command::new(&objcopy);
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

    let mut cmd = Command::new(&objcopy);
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
            cmd: objcopy.to_string() 
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
    let mut cmd = Command::new(&ld);
    for file in link_files {
        cmd.arg(file);
    }
    cmd
        .arg("-T")
        .arg(link_file)
        .arg("-e")
        .arg(&alloc_info.kernel_entry.to_string())
        .arg("-o")
        .arg(outfile)
        .arg("-z")
        .arg("noexecstack");
    check_cmd(cmd).map_err(|_| PkgError::CmdError { cmd: ld.to_string() })?;
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
            eprintln!("{}", err);
            exit(1);
        }
    }
}
