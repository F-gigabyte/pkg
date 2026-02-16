use core::fmt;
use std::{cmp::Ordering, collections::HashMap, env, fs::{self, File}, io::{self, Read}, path::Path, process::{Command, exit}};
use serde::Deserialize;

use object::{Endianness, Object, ObjectKind, ObjectSection, ObjectSymbol, SectionIndex, StringTable, elf::{SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, STT_FUNC}, read::elf::{ElfFile, ElfFile32, FileHeader, SectionHeader}};

const MIN_SIZE: u32 = 256;

const BOOTLOADER_ADDR: usize = 0x10000000;
const VECTORS_ADDR: usize = 0x10000000 + 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionAttr {
    attr: u8,
}

impl SectionAttr {
    pub const READ_MASK: u8 = 0b100;
    pub const WRITE_MASK: u8 = 0b010;
    pub const EXEC_MASK: u8 = 0b001;

    pub fn new(read: bool, write: bool, exec: bool) -> Self {
        let mut attr = 0;
        if read {
            attr |= Self::READ_MASK;
        }
        if write {
            attr |= Self::WRITE_MASK;
        }
        if exec {
            attr |= Self::EXEC_MASK;
        }
        Self {
            attr
        }
    }

    pub fn read(&self) -> bool {
        self.attr & Self::READ_MASK != 0
    }
    
    pub fn write(&self) -> bool {
        self.attr & Self::WRITE_MASK != 0
    }
    
    pub fn exec(&self) -> bool {
        self.attr & Self::EXEC_MASK != 0
    }

    pub fn set_read(&mut self, state: bool) {
        if state {
            self.attr |= Self::READ_MASK;
        } else {
            self.attr &= !Self::READ_MASK;
        }
    }
    
    pub fn set_write(&mut self, state: bool) {
        if state {
            self.attr |= Self::WRITE_MASK;
        } else {
            self.attr &= !Self::WRITE_MASK;
        }
    }
    
    pub fn set_exec(&mut self, state: bool) {
        if state {
            self.attr |= Self::EXEC_MASK;
        } else {
            self.attr &= !Self::EXEC_MASK;
        }
    }
}

impl fmt::Display for SectionAttr {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.read() {
            write!(fmt, "r")?;
        } else {
            write!(fmt, "-")?;
        }
        if self.write() {
            write!(fmt, "w")?;
        } else {
            write!(fmt, "-")?;
        }
        if self.exec() {
            write!(fmt, "x")?;
        } else {
            write!(fmt, "-")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionAttr {
    RX = 0b101,
    R = 0b100,
    RW = 0b110
}

impl TryFrom<SectionAttr> for RegionAttr {
    type Error = SectionAttr;
    fn try_from(value: SectionAttr) -> Result<Self, Self::Error> {
        if value.read() {
            if value.write() {
                Ok(Self::RW)
            } else if value.exec() {
                Ok(Self::RX)
            } else {
                Ok(Self::R)
            }
        } else {
            Err(value)
        }
    }
}

impl fmt::Display for RegionAttr {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::RX => write!(fmt, "rx"),
            Self::R => write!(fmt, "r"),
            Self::RW => write!(fmt, "rw")
        }
    }
}

#[derive(Debug)]
pub enum PkgError {
    ReadError(String),
    WriteError(String),
    MkdirError(String),
    ParseError(String),
    NoStringTable(String),
    NonRelocatable(String),
    NoSpace(String, String),
    TooManySections(String),
    InvalidArgs,
    NoKernelEntry,
    NoKernelStack,
    NoProgramEntry(String),
    NoProgramStack(String),
    CmdError(String),
    InvalidRegionPermissions(String, String, SectionAttr),
    NoProgram(String)
}

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
    attr: RegionAttr,
    load: bool,
    entry_addr: Option<usize>,
    size: usize
}

#[derive(Debug)]
pub struct Region {
    phys_addr: u32,
    virt_addr: u32,
    len: u32
}

impl Region {
    pub const fn default() -> Self {
        Self { phys_addr: 0, virt_addr: 0, len: 0 }
    }

    fn serialise(&self) -> Vec<u8> {
        let mut res = Vec::new();
        res.extend_from_slice(&self.phys_addr.to_ne_bytes());
        res.extend_from_slice(&self.virt_addr.to_ne_bytes());
        res.extend_from_slice(&self.len.to_ne_bytes());
        res
    }
}

#[derive(Debug)]
pub struct Program {
    name: String,
    priority: u8,
    driver: u16,
    sp: Option<u32>,
    entry: Option<u32>,
    regions: [Region; 8]
}

impl Program {
    fn find_empty_region(&mut self) -> Option<&mut Region> {
        for region in self.regions.iter_mut() {
            if region.virt_addr & 1 == 0 {
                return Some(region);
            }
        }
        None
    }

    fn serialise(&self) -> Result<Vec<u8>, PkgError> {
        let mut res = Vec::new();
        res.extend_from_slice(&(self.priority as u32 | ((self.driver as u32) << 16)).to_le_bytes());
        res.extend_from_slice(&self.sp.ok_or(PkgError::NoProgramStack(self.name.to_string()))?.to_le_bytes());
        res.extend_from_slice(&self.entry.ok_or(PkgError::NoProgramEntry(self.name.to_string()))?.to_le_bytes());
        for region in &self.regions {
            res.extend(region.serialise().iter());
        }
        Ok(res)
    }
}

pub struct SectionRename {
    old_name: String,
    new_name: String,
}

pub struct Section {
    name: String,
    phys_addr: usize,
    virt_addr: usize
}

fn do_allocs(allocs: Vec<Alloc>, ram: &mut Vec<Block>, flash: &mut Vec<Block>, sections: &mut Vec<Section>, programs: &mut HashMap<String, Program>) -> Result<(usize, usize), PkgError> {
    let mut kernel_entry = None;
    let mut kernel_stack = None;
    for mut alloc in allocs {
        let is_kernel = if alloc.name == "kernel" { 
            true
        } else {
            false
        };
        if !is_kernel {
            alloc.size = alloc.size.next_power_of_two().max(MIN_SIZE as usize);
        }
        let mut virt_addr = None;
        let attr = alloc.attr;
        let len = alloc.size;
        let alloc_name = alloc.name.clone();
        let region = alloc.region.clone();
        let name = format!("{}{}", alloc.name, alloc.region);
        let entry_addr = alloc.entry_addr.take();
        if alloc.load {
            virt_addr = Some(try_alloc_with_alloc(alloc.clone(), is_kernel, ram).map_err(|_| PkgError::NoSpace(alloc_name.clone(), region.clone()))?);
        }
        let mut phys_addr = None;
        if alloc.region != ".stack" {
            if is_kernel && alloc.region == ".bootloader" {
                try_alloc(
                    Block { 
                        lower: BOOTLOADER_ADDR, 
                        upper: BOOTLOADER_ADDR + alloc.size, 
                        region: format!("{}{} ({})", alloc_name.clone(), region.clone(), alloc.attr)
                    },
                    flash
                ).map_err(|_| PkgError::NoSpace(alloc_name.clone(), region.clone()))?;
                phys_addr = Some(BOOTLOADER_ADDR);
            } else if is_kernel && alloc.region == ".text.vectors" {
                try_alloc(
                    Block { 
                        lower: VECTORS_ADDR, 
                        upper: VECTORS_ADDR + alloc.size, 
                        region: format!("{}{} ({})", alloc_name.clone(), region.clone(), alloc.attr)
                    },
                    flash
                ).map_err(|_| PkgError::NoSpace(alloc_name.clone(), region.clone()))?;
                phys_addr = Some(VECTORS_ADDR);
            } else {
                phys_addr = Some(try_alloc_with_alloc(alloc, is_kernel, flash).map_err(|_| PkgError::NoSpace(alloc_name.clone(), region.clone()))?);
            }
        }
        if virt_addr.is_none() && phys_addr.is_none() {
            continue;
        }
        let virt_addr = virt_addr.unwrap_or_else(|| phys_addr.unwrap());
        let phys_addr = phys_addr.unwrap_or(virt_addr);
        if region != ".stack" {
            let alloc_sec = Section {
                name,
                phys_addr,
                virt_addr
            };
            sections.push(alloc_sec);
        }
        if !is_kernel {
            if let Some(program) = programs.get_mut(&alloc_name) {
                let sec = program.find_empty_region().ok_or(PkgError::TooManySections(alloc_name.to_string()))?;
                sec.len = len as u32;
                sec.phys_addr = phys_addr as u32;
                sec.virt_addr = virt_addr as u32 | ((attr as u32) << 1) | 1;
                if region == ".stack" {
                    program.sp = Some(virt_addr as u32 + len as u32);
                }
                if let Some(entry_addr) = entry_addr {
                    program.entry = Some(entry_addr as u32 + virt_addr as u32);
                }
            } else {
                return Err(PkgError::NoProgram(alloc_name.to_string()));
            }
        } else {
            if let Some(entry_addr) = entry_addr {
                kernel_entry = Some(entry_addr + virt_addr);
            }
            if region == ".stack" {
                kernel_stack = Some(virt_addr + len);
            }
        }
    }
    Ok((kernel_entry.ok_or(PkgError::NoKernelEntry)?, kernel_stack.ok_or(PkgError::NoKernelStack)?))
}

fn get_file_regions(name: &str, file: &LoadedConfig, allocs: &mut Vec<Alloc>, renames: &mut HashMap<String, (Vec<SectionRename>, Vec<String>)>) -> Result<(), PkgError> {
    // check relocatable
    if file.data.kind() != ObjectKind::Relocatable {
        return Err(PkgError::NonRelocatable(file.filename.to_string()));
    }
    // get string table to get section names
    let index = file.data.elf_header().e_shstrndx(Endianness::Little);
    let string_sec = file.data.section_by_index(SectionIndex(index as usize)).map_err(|_| PkgError::NoStringTable(file.filename.to_string()))?;
    let data = string_sec.data().map_err(|_| PkgError::NoStringTable(file.filename.to_string()))?;
    let string_table = StringTable::new(data, 0, data.len() as u64);
    let mut file_secs = Vec::new();
    let entry_addr = file.data.entry() as usize;
    
    let mut file_symbols = Vec::new();

    let mut entry_sec = None;

    for symbol in file.data.symbols() {
        let name = symbol.name().map_err(|_| PkgError::NoStringTable(file.filename.to_string()))?.to_string();
        let addr = symbol.address() as usize;
        if addr == entry_addr && symbol.elf_symbol().st_type() == STT_FUNC {
            entry_sec = symbol.section_index();
        }
        file_symbols.push(name);
    }

    for (i, sec) in file.data.elf_section_table().iter().enumerate() {
        if sec.sh_flags(Endianness::Little) & SHF_ALLOC != 0 {
            let region_name = String::from_utf8_lossy(sec.name(Endianness::Little, string_table).map_err(|_| PkgError::NoStringTable(file.filename.to_string()))?);
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
            // put section to be allocated later
            allocs.push(Alloc {
                name: name.to_string(),
                region: region_name.to_string(),
                attr: RegionAttr::try_from(flags).map_err(|err| PkgError::InvalidRegionPermissions(name.to_string(), region_name.to_string(), err))?,
                load,
                entry_addr,
                size
            });
        }
    }



    if let Some(stack) = file.data.symbol_by_name("__stack_size") {
        // if have reserved a stack, allocate this as well
        allocs.push(Alloc { 
            name: name.to_string(),
            region: ".stack".to_string(), 
            attr: RegionAttr::RW, 
            load: true, 
            entry_addr: None,
            size: stack.address() as usize 
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

fn try_alloc_with_alloc(alloc: Alloc, is_kernel: bool, region: &mut Vec<Block>) -> Result<usize, Alloc> {
    let mut suggest = None;
    let mut overflow = 0;
    let alignment = if is_kernel {
        4
    } else {
        alloc.size
    };
    for i in 0..region.len() {
        if region[i].region == "free" {
            let lower = (region[i].lower + alignment - 1) & !(alignment - 1);
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
        let lower = (region[suggest].lower + alignment - 1) & !(alignment - 1);
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
    check_cmd(cmd).map_err(|_| PkgError::CmdError(objcopy.to_string()))?; 
    Ok(res_name)
}

fn create_link_file(path: &Path, sections: &Vec<Section>, prog_table_file: &str, prog_table_phys: usize, kernel_stack: usize) -> Result<String, PkgError> {
    let mut link_data = String::new();
    link_data.push_str("ENTRY(_start);\nSECTIONS {");
    for sec in sections {
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
    link_data = format!("{}\n\tprogram_table 0x{:x} : AT(0x{:x}) {{\n\t\t__program_table = .;\n\t\t{}\n\t}}", link_data, prog_table_phys, prog_table_phys, prog_table_file);
    link_data = format!("{}\n\t__kernel_stack = 0x{:x};", link_data, kernel_stack);
    link_data.push_str("\n}\n");
    let link_file = path.join("link.ld");
    fs::write(&link_file, link_data).map_err(|_| PkgError::WriteError("link.ld".to_string()))?;
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
struct FileConfig {
    priority: u8,
    driver: u16,
    debug_src: String,
    release_src: String,
}

struct LoadedConfig<'a> {
    filename: String,
    data: ElfFile32<'a>
}

fn run(args: Vec<String>) -> Result<(), PkgError> {
    let objcopy = env::var("OBJCOPY").unwrap_or("objcopy".to_string());
    let ld = env::var("LD").unwrap_or("ld".to_string());
    let mut config_index = 1;
    let mut sections = Vec::new();
    if args.len() < 4 || args.len() > 5 {
        return Err(PkgError::InvalidArgs);
    }
    let debug = if args.len() == 5 {
        match args[1].as_ref() {
            "-r" => {
                config_index = 2;
                false
            },
            _ => {
                return Err(PkgError::InvalidArgs);
            }
        }
    } else {
        true
    };
    if args[config_index + 1] != "-o" {
        return Err(PkgError::InvalidArgs);
    }
    let outfile = &args[config_index + 2];
    let config_file = &args[config_index];
    let config = match fs::read_to_string(config_file) {
        Ok(res) => res,
        Err(_) => {
            eprintln!("Unable to read file {}.", config_file);
            exit(1);
        }
    };
    let config = match toml::from_str::<HashMap<String, FileConfig>>(&config) {
        Ok(res) => res,
        Err(_) => {
            eprintln!("Unable to parse file {}.", config_file);
            exit(1);
        }
    };
    let mut renames = HashMap::new();
    let mut file_data = HashMap::new();
    let mut programs = HashMap::new();
    for (name, config) in config {
        if name != "kernel" {
            programs.insert(
                name.to_string(), 
                Program { 
                    name: name.to_string(),
                    priority: config.priority, 
                    driver: config.driver,
                    sp: None, 
                    entry: None, 
                    regions: [const { Region::default() }; 8] 
                }
            );
        }
        let mut data = Vec::new();
        let filename = if debug {
            config.debug_src
        } else {
            config.release_src
        };
        let mut file = File::open(&filename).map_err(|_| PkgError::ReadError(filename.to_string()))?;
        file.read_to_end(&mut data).map_err(|_| PkgError::ReadError(filename.to_string()))?;
        file_data.insert(name, (filename, data));
    }
    let mut files = HashMap::new();
    for (name, (filename, data)) in &file_data {
        let data = ElfFile::parse(data.as_ref()).map_err(|_| PkgError::ReadError(filename.to_string()))?;
        files.insert(name.to_string(), LoadedConfig {
            filename: filename.to_string(),
            data
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
    // sort allocs from smallest region to largest region
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
                } else {
                    Ordering::Equal 
                }
            },
            (true, false) => {
                Ordering::Less
            },
            (false, true) => {
                Ordering::Greater
            },
            (false, false) => {
                if a.size == b.size { 
                    Ordering::Equal 
                } else if a.size < b.size { 
                    Ordering::Less 
                } else { 
                    Ordering::Greater 
                }
            }
        }
    );
    let (kernel_entry, kernel_stack) = do_allocs(allocs, &mut ram, &mut flash, &mut sections, &mut programs)?;
    let username = whoami::username().unwrap();
    let path = format!("/tmp/pkg_{}", username);
    let path = Path::new(&path);
    if !path.exists() {
        fs::create_dir(path).map_err(|_| PkgError::MkdirError(path.to_str().unwrap().to_string()))?;
    }
    let mut link_files = Vec::new();
    print_renames(&renames);
    for (file, (secs, symbols)) in renames {
        rename_file_sections(&objcopy, path, &file, &secs, &symbols, &mut link_files)?;
        // based on answer by embradded on https://stackoverflow.com/questions/68622938/new-versions-of-ld-cannot-take-elf-files-as-input-to-link accessed 11/02/2026
    }
    let mut prog_table_bytes = Vec::new();
    prog_table_bytes.extend_from_slice(&(programs.len() as u32).to_ne_bytes());
    for (_, prog) in &programs {
        prog_table_bytes.extend(prog.serialise()?.iter());
    }
    let prog_table_size = prog_table_bytes.len();
    let prog_table_file_bin = path.join("prog_table.bin");
    let prog_table_file = path.join("prog_table.o").to_string_lossy().to_string().to_string();
    fs::write(&prog_table_file_bin, prog_table_bytes).map_err(|_| PkgError::WriteError(prog_table_file_bin.to_str().unwrap().to_string()))?;
    let prog_table_file_bin = prog_table_file_bin.to_string_lossy().to_string().to_string();
    let prog_table_alloc = Alloc {
        name: "program_table".to_string(),
        region: ".program_table".to_string(),
        attr: RegionAttr::R,
        load: false,
        entry_addr: None,
        size: prog_table_size
    };
    let prog_table_phys = try_alloc_with_alloc(prog_table_alloc, true, &mut flash).map_err(|_| PkgError::NoSpace("program_table".to_string(), ".program_table".to_string()))?;
    println!("RAM");
    print_mem_map(&ram);
    println!("Flash");
    print_mem_map(&flash);
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
    check_cmd(cmd).map_err(|_| PkgError::CmdError(objcopy.to_string()))?;
    let link_file = create_link_file(&path, &sections, &prog_table_file, prog_table_phys, kernel_stack)?;
    let mut cmd = Command::new(&ld);
    for file in link_files {
        cmd.arg(file);
    }
    cmd
        .arg("-T")
        .arg(link_file)
        .arg("-e")
        .arg(&kernel_entry.to_string())
        .arg("-o")
        .arg(outfile)
        .arg("-z")
        .arg("noexecstack");
    check_cmd(cmd).map_err(|_| PkgError::CmdError(ld.to_string()))?;
    for program in programs {
        println!("{:#x?}", program);
    }
    Ok(())

}

fn main() {
    let args: Vec<_> = env::args().collect();
    let name = args[0].clone();
    match run(args) {
        Ok(()) => {},
        Err(err) => {
            match err {
                PkgError::InvalidArgs => {
                    eprintln!("Usage: {} [-r] config -o outfile", name);
                },
                PkgError::ReadError(file) => {
                    eprintln!("Error reading file '{}'.", file);
                },
                PkgError::ParseError(file) => {
                    eprintln!("Error parsing file '{}'.", file);
                },
                PkgError::NoStringTable(file) => {
                    eprintln!("{} has no string table.", file);
                },
                PkgError::NonRelocatable(file) => {
                    eprintln!("{} is non relocatable.", file);
                },
                PkgError::NoSpace(name, region) => {
                    eprintln!("No space for {} region '{}'.", name, region);
                },
                PkgError::CmdError(cmd) => {
                    eprintln!("Error running command '{}'.", cmd);
                },
                PkgError::TooManySections(name) => {
                    eprintln!("{} has too many sections.", name);
                },
                PkgError::NoKernelEntry => {
                    eprintln!("No kernel entry address.");
                },
                PkgError::NoKernelStack => {
                    eprintln!("No kernel entry stack.");
                },
                PkgError::WriteError(file) => {
                    eprintln!("Error writing to file '{}'.", file);
                },
                PkgError::MkdirError(path) => {
                    eprintln!("Error creating directory '{}'.", path);
                },
                PkgError::NoProgramEntry(name) => {
                    eprintln!("No entry address for program '{}'.", name);
                },
                PkgError::NoProgramStack(name) => {
                    eprintln!("No stack for program '{}'.", name);
                },
                PkgError::NoProgram(name) => {
                    eprintln!("No program with name '{}'.", name);
                },
                PkgError::InvalidRegionPermissions(name, region, flags) => {
                    eprintln!("Invalid region permissions for {} region {} of '{}'.", name, region, flags);
                }
            }
            exit(1);
        }
    }
}
