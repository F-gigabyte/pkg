use std::{cmp::Ordering, collections::{HashMap, HashSet}, env, fmt, fs::{self, File}, io::{self, Read}, path::Path, process::{Command, ExitStatus}};

use object::{Endian, Endianness, Object, ObjectKind, ObjectSection, ObjectSymbol, SectionFlags, SectionIndex, StringTable, elf::{PT_LOAD, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE}, read::elf::{ElfFile, ElfFile32, ElfFile64, FileHeader, HashTable, ProgramHeader, SectionHeader}};

const MIN_SIZE: u32 = 256;

#[derive(Debug, Clone, Copy)]
pub enum FileError {
    Read,
    NoStringTable,
    NonRelocatable,
    NoSpace,
    TooManySections,
    NoProcTable
}

#[derive(Debug)]
pub struct Block {
    lower: usize,
    upper: usize,
    region: String
}

pub struct PHeader {
    phys_addr: usize,
    virt_addr: usize,
    len: usize
}

#[derive(Clone)]
pub struct Alloc {
    filename: String,
    region: String,
    attr: String,
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
    priority: u32,
    sp: u32,
    entry: u32,
    regions: [Region; 8]
}

impl Program {
    fn find_empty_region(&mut self) -> Option<&mut Region> {
        for region in self.regions.iter_mut() {
            if region.len == 0 {
                return Some(region);
            }
        }
        None
    }

    fn serialise(&self) -> Vec<u8> {
        let mut res = Vec::new();
        res.extend_from_slice(&self.priority.to_ne_bytes());
        res.extend_from_slice(&self.sp.to_ne_bytes());
        res.extend_from_slice(&self.entry.to_ne_bytes());
        for region in &self.regions {
            res.extend(region.serialise().iter());
        }
        res
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

fn do_allocs(allocs: Vec<Alloc>, ram: &mut Vec<Block>, flash: &mut Vec<Block>, sections: &mut Vec<Section>, programs: &mut HashMap<String, Program>) -> Result<(), FileError> {
    for mut alloc in allocs {
        let mut virt_addr = None;
        let filename = alloc.filename.clone();
        let attr = alloc.attr.clone();
        let len = alloc.size;
        let name = format!("{}{}", alloc.filename, alloc.region);
        let entry_addr = alloc.entry_addr.take();
        if alloc.load {
            virt_addr = Some(try_alloc_with_alloc(alloc.clone(), ram).map_err(|_| FileError::NoSpace)?);
        }
        let mut phys_addr = None;
        let region = alloc.region.clone();
        if alloc.region != ".stack" {
            phys_addr = Some(try_alloc_with_alloc(alloc, flash).map_err(|_| FileError::NoSpace)?);
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
        if let Some(program) = programs.get_mut(&filename) {
            let sec = program.find_empty_region().ok_or(FileError::TooManySections)?;
            sec.len = len as u32;
            sec.phys_addr = phys_addr as u32;
            sec.virt_addr = virt_addr as u32 | if attr == "x" { 1 } else { 0 };
            if region == ".stack" {
                program.sp = virt_addr as u32 + len as u32;
            }
            if let Some(entry_addr) = entry_addr {
                program.entry = entry_addr as u32 + virt_addr as u32;
            }
        } else {
            let mut program = Program { 
                priority: 0xbebafeca, 
                sp: 0, 
                entry: 0, 
                regions: [const { Region::default() }; 8] 
            };
            program.regions[0].len = len as u32;
            program.regions[0].phys_addr = phys_addr as u32;
            program.regions[0].virt_addr = virt_addr as u32 | if attr == "x" { 1 } else { 0 };
            if region == ".stack" {
                program.sp = virt_addr as u32 + len as u32;
            }
            if let Some(entry_addr) = entry_addr {
                program.entry = entry_addr as u32 + virt_addr as u32;
            }
            programs.insert(
                filename, 
                program
            );
        }
    }
    Ok(())
}

fn get_file_regions(file: &ElfFile32, filename: &str, allocs: &mut Vec<Alloc>, renames: &mut HashMap<String, (Vec<SectionRename>, Vec<(String, u64)>)>) -> Result<(), FileError> {
    // check relocatable
    if file.kind() != ObjectKind::Relocatable {
        return Err(FileError::NonRelocatable);
    }
    // get string table to get section names
    let index = file.elf_header().e_shstrndx(Endianness::Little);
    let string_sec = file.section_by_index(SectionIndex(index as usize)).map_err(|_| FileError::NoStringTable)?;
    let data = string_sec.data().map_err(|_| FileError::NoStringTable)?;
    let string_table = StringTable::new(data, 0, data.len() as u64);
    let mut file_secs = Vec::new();
    let entry_addr = file.entry() as usize;
    for sec in file.elf_section_table().iter() {
        if sec.sh_flags(Endianness::Little) & SHF_ALLOC != 0 {
            let name = String::from_utf8_lossy(sec.name(Endianness::Little, string_table).map_err(|_| FileError::NoStringTable)?);
            let sec_size = sec.sh_size(Endianness::Little) as usize;
            let size = sec_size as usize;
            let addr = sec.sh_addr(Endianness::Little) as usize;
            // get section flags
            let flags = if sec.sh_flags(Endianness::Little) & SHF_EXECINSTR != 0 {
                "x"
            } else {
                "rw"
            };
            let load = if sec.sh_flags(Endianness::Little) & SHF_WRITE != 0 {
                true
            } else {
                false
            };
            // rename section to be filename.section_name so can produce a linker script for it
            // later 
            file_secs.push(SectionRename {
                old_name: name.to_string(),
                new_name: format!("{}{}", filename, name)
            });
            let entry_addr = if entry_addr >= addr && entry_addr < addr + sec_size {
                Some(entry_addr - addr)
            } else {
                None
            };
            // put section to be allocated later
            allocs.push(Alloc {
                filename: filename.to_string(),
                region: name.to_string(),
                attr: flags.to_string(),
                load,
                entry_addr,
                size
            });
        }
    }

    let mut file_symbols = Vec::new();

    for symbol in file.symbols() {
        let name = symbol.name().unwrap().to_string();
        let addr = symbol.address();
        println!("Have symbol {} at 0x{:x}", name, addr);
        file_symbols.push((name, addr));
    }

    if let Some(stack) = file.symbol_by_name("__stack_size") {
        // if have reserved a stack, allocate this as well
        allocs.push(Alloc { 
            filename: filename.to_string(), 
            region: ".stack".to_string(), 
            attr: "rw".to_string(), 
            load: true, 
            entry_addr: None,
            size: stack.address() as usize 
        });
    }
    // add section renames to hash map
    _ = renames.insert(filename.to_string(), (file_secs, file_symbols));
    Ok(())
}

fn load_kernel_regions(kernel: &ElfFile32, filename: &str, allocs: &mut Vec<Alloc>, sections: &mut Vec<Section>, renames: &mut HashMap<String, (Vec<SectionRename>, Vec<(String, u64)>)>) -> Result<u32, FileError> {
    // check relocatable
    if kernel.kind() != ObjectKind::Relocatable {
        return Err(FileError::NonRelocatable);
    }
    let index = kernel.elf_header().e_shstrndx(Endianness::Little);
    let string_sec = kernel.section_by_index(SectionIndex(index as usize)).map_err(|_| FileError::NoStringTable)?;
    let data = string_sec.data().map_err(|_| FileError::NoStringTable)?;
    let string_table = StringTable::new(data, 0, data.len() as u64);
    let mut file_secs = Vec::new();
    for sec in kernel.elf_section_table().iter() {
        if sec.sh_flags(Endianness::Little) & SHF_ALLOC != 0 {
            let virt_addr = sec.sh_addr(Endianness::Little) as usize;
            let phys_addr = find_physaddr(&program_headers, virt_addr).unwrap();
            let name = String::from_utf8_lossy(sec.name(Endianness::Little, string_table).map_err(|_| FileError::NoStringTable)?).to_string();
            let res_name = format!("kernel{}", name);
            file_secs.push(SectionRename {
                old_name: name,
                new_name: res_name.clone()
            });
            sections.push(Section { 
                name: res_name, 
                phys_addr, 
                virt_addr 
            });
        }
    }
    let kernel_stack = Block {
        lower: 0x20000000,
        upper: 0x20002000,
        region: "kernel".to_string()
    };
    try_alloc(kernel_stack, ram).map_err(|_| FileError::NoSpace)?;
    let mut kernel_symbols = Vec::new();
    for symbol in kernel.symbols() {
        let name = symbol.name().unwrap().to_string();
        let addr = symbol.address();
        println!("Have symbol {} at 0x{:x}", name, addr);
        kernel_symbols.push((name, addr));
    }
    _ = renames.insert(filename.to_string(), (file_secs, kernel_symbols));
    let proc_table = kernel.symbol_by_name("__program_table").ok_or(FileError::NoProcTable)?;
    let addr = proc_table.address() as u32;
    Ok(addr)
}


fn check_cmd(mut cmd: Command) -> Result<(), i32> {
    let status = cmd.output().unwrap().status.code().unwrap();
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
    let mut suggest = None;
    let mut overflow = 0;
    for i in 0..region.len() {
        if region[i].region == "free" {
            let lower = (region[i].lower + alloc.size - 1) & !(alloc.size - 1);
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
        let lower = (region[suggest].lower + alloc.size - 1) & !(alloc.size - 1);
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
            region: format!("{}{} ({})", alloc.filename, alloc.region, alloc.attr)
        };
        Ok(lower)
    } else {
        Err(alloc)
    }
}

fn rename_file_sections(objcopy: &str, path: &Path, file: &str, sections: &Vec<SectionRename>, symbols: &Vec<(String, u64)>, link_files: &mut Vec<String>) -> Result<String, i32> {
    let file_name = Path::new(&file).file_name().unwrap().to_string_lossy().to_string();
    let res_name = path.join(&file_name).to_string_lossy().to_string().to_string();
    link_files.push(res_name.clone());
    let mut cmd = Command::new(&objcopy);
    for sec in sections {
        cmd.arg("--rename-section").arg(&format!("{}={}", sec.old_name, sec.new_name));
    }
    cmd.arg("--strip-all");
    cmd.arg(&file).arg(&res_name);
    check_cmd(cmd)?; 
    Ok(res_name)
}

fn create_link_file(path: &Path, sections: &Vec<Section>, prog_table_file: &str, prog_table_phys: usize) -> Result<String, io::Error> {
    let mut link_data = String::new();
    link_data.push_str("ENTRY(_start);\nSECTIONS {");
    for sec in sections {
        link_data = format!("{}\n\t{} 0x{:x} : AT(0x{:x}) {{\n\t\t*({});\n\t}}", link_data, sec.name, sec.virt_addr, sec.phys_addr, sec.name);
    }
    link_data = format!("{}\n\tprogram_table 0x{:x} : AT(0x{:x}) {{\n\t\t{}\n\t}}", link_data, prog_table_phys, prog_table_phys, prog_table_file);
    link_data.push_str("\n}\n");
    let link_file = path.join("link.ld");
    fs::write(&link_file, link_data)?;
    let link_file = link_file.to_string_lossy().to_string().to_string();
    Ok(link_file)
}

fn print_mem_map(region: &Vec<Block>) {
    for block in region {
        println!("0x{:x} -> 0x{:x}: {}", block.lower, block.upper, block.region);
    }
}

fn print_renames(renames: &HashMap<String, (Vec<SectionRename>, Vec<(String, u64)>)>) {
    for (file, (secs, _)) in renames.iter() {
        println!("{}", file);
        for sec in secs {
            println!("\t{} -> {}", sec.old_name, sec.new_name);
        }
    }
}

fn find_physaddr(program_headers: &Vec<PHeader>, virt_addr: usize) -> Option<usize> {
    for header in program_headers {
        if header.virt_addr <= virt_addr && header.virt_addr + header.len > virt_addr {
            return Some(virt_addr - header.virt_addr + header.phys_addr);
        }
    }
    None
}

fn main() -> Result<(), FileError> {
    let objcopy = env::var("OBJCOPY").unwrap_or("objcopy".to_string());
    let ld = env::var("LD").unwrap_or("ld".to_string());
    let mut args = env::args();
    let mut sections = Vec::new();
    args.next(); // pkg
    let file_names: Vec<_> = args.collect();
    let file_names: Vec<_> = file_names.iter().map(|arg| Path::new(arg)).collect();
    let mut renames = HashMap::new();
    let file_data: Vec<_> = file_names.iter().map(|arg| -> Result<Vec<u8>, FileError> {
        let mut data = Vec::new();
        let mut file = File::open(arg).map_err(|_| FileError::Read)?;
        file.read_to_end(&mut data).map_err(|_| FileError::Read)?;
        Ok(data)
    }).collect();
    let mut files = file_data.iter().map(|data| -> Result<ElfFile32, FileError> {
        let data = data.as_ref().map_err(|err| *err)?;
        ElfFile::parse(data.as_ref()).map_err(|_| FileError::Read)
    });
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
    let kernel = files.next().unwrap();
    let kernel = kernel?;
    let kernel_name = file_names[0].to_string_lossy().to_string().to_string();
    let entry_addr = kernel.entry() as usize;
    let program_table = load_kernel_regions(&kernel, &kernel_name, &mut flash, &mut ram, &mut sections, &mut renames)?;
    // program table size, subtract one as kernel not included and add 4 for the 4 length bytes
    let program_size = std::mem::size_of::<Program>() * (file_names.len() - 1) + 4;
    let mut programs = HashMap::new();
    try_alloc(
        Block { 
            lower: program_table as usize, 
            upper: program_table as usize + program_size, 
            region: "program_table".to_string() 
        }, 
        &mut flash
    ).unwrap();
    let mut allocs = Vec::new();
    for (i, file) in files.enumerate() {
        let file = file?;
        let filename = &file_names[i + 1].to_string_lossy().to_string().to_string();
        get_file_regions(&file, &filename, &mut allocs, &mut renames)?;
    }
    // sort allocs from smallest region to largest region
    allocs.sort_by(|a, b| if a.size == b.size { Ordering::Equal } else if a.size < b.size { Ordering::Less } else { Ordering::Greater });
    do_allocs(allocs, &mut ram, &mut flash, &mut sections, &mut programs)?;
    println!("RAM");
    print_mem_map(&ram);
    println!("Flash");
    print_mem_map(&flash);
    print_renames(&renames);
    let username = whoami::username().unwrap();
    let path = format!("/tmp/pkg_{}", username);
    let path = Path::new(&path);
    if !path.exists() {
        fs::create_dir(path).unwrap();
    }
    let mut link_files = Vec::new();
    for (file, (secs, symbols)) in renames {
        let res_name = rename_file_sections(&objcopy, path, &file, &secs, &symbols, &mut link_files).unwrap();
        // for kernel 0 out 16 and 17 bytes
        // based on answer by embradded on https://stackoverflow.com/questions/68622938/new-versions-of-ld-cannot-take-elf-files-as-input-to-link accessed 11/02/2026
        if *file == kernel_name {
            let mut cmd = Command::new("dd");
            cmd.arg("if=/dev/zero")
                .arg(&format!("of={}", res_name))
                .arg("bs=1")
                .arg("seek=16")
                .arg("conv=notrunc")
                .arg("count=2");
            check_cmd(cmd).unwrap();
        }
    }
    let mut prog_table_bytes = Vec::new();
    prog_table_bytes.extend_from_slice(&(programs.len() as u32).to_ne_bytes());
    for (_, prog) in &programs {
        prog_table_bytes.extend(prog.serialise().iter());
    }
    let prog_table_file_bin = path.join("prog_table.bin");
    let prog_table_file = path.join("prog_table.o").to_string_lossy().to_string().to_string();
    fs::write(&prog_table_file_bin, prog_table_bytes).unwrap();
    let prog_table_file_bin = prog_table_file_bin.to_string_lossy().to_string().to_string();
    let mut cmd = Command::new(objcopy);
    cmd
        .arg("-O")
        .arg("elf32-littlearm")
        .arg("-I")
        .arg("binary")
        .arg("-B")
        .arg("arm")
        .arg(&prog_table_file_bin)
        .arg(&prog_table_file);
    check_cmd(cmd).unwrap();
    let link_file = create_link_file(&path, &sections, &prog_table_file, program_table as usize).unwrap();
    let mut cmd = Command::new(ld);
    for file in link_files {
        cmd.arg(file);
    }
    cmd
        .arg("-T")
        .arg(link_file)
        .arg("-e")
        .arg(entry_addr.to_string())
        .arg("-o")
        .arg("small_os")
        .arg("-z")
        .arg("noexecstack");
    check_cmd(cmd).unwrap();
    for program in programs {
        println!("{:#x?}", program);
    }

    Ok(())
}
