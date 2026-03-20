use std::{collections::{HashMap, VecDeque}, fs::File, io::{Read, Write}, mem};

use hamming::calc_symbol_len;
use crc32::calc_crc;
use object::{Endianness, Object, ObjectKind, ObjectSection, ObjectSymbol, SectionIndex, StringTable, elf::{SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, STT_FUNC}, read::elf::{ElfFile, ElfFile32, FileHeader, ProgramHeader, SectionHeader}};

use crate::{allocs::{Alloc, AllocType}, driver_args::DriverArgs, errors::PkgError, file_config::LoadedConfig, program::Program, region::Region, region_attr::RegionAttr, section_attr::SectionAttr, sections::SectionRename};

const MIN_REGION_SIZE: u32 = 256;

fn round_word(val: usize) -> usize {
    ((val + mem::size_of::<u32>() - 1) / mem::size_of::<u32>()) * mem::size_of::<u32>()
}

fn get_file_offset_from_phys_addr(file: &ElfFile32, phys_addr: usize, len: usize) -> Option<usize> {
    let end_addr = phys_addr + len;
    for header in file.elf_program_headers() {
        let header_phys = header.p_paddr(Endianness::Little) as usize;
        let header_phys_end = header_phys + header.p_memsz(Endianness::Little) as usize;
        if header_phys <= phys_addr && header_phys_end >= end_addr {
            let file_offset = phys_addr - header_phys;
            return Some(header.p_offset(Endianness::Little) as usize + file_offset);
        }
    }
    None
}

pub fn add_final_crcs(filename: &str, outfile: &str) -> Result<(), PkgError> {
    let mut data = Vec::new();
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
    let file: ElfFile32 = ElfFile::parse(data.as_ref()).map_err(|_| 
        PkgError::ReadError {
            file: filename.to_string()
        }
    )?;
    let mut update_table = Vec::new();
    let program_table = file.section_by_name("program_table").unwrap();
    let program_table_data = program_table.data().unwrap();
    let num_programs = u32::from_le_bytes(program_table_data[..mem::size_of::<u32>()].try_into().unwrap());
    for i in 0..num_programs {
        let program_offset = 4 + Program::get_prog_size() * (i as usize);
        let regions_offset = program_offset + 5 * mem::size_of::<u32>();
        for j in 0..8 {
            let region_offset = regions_offset + j * Region::get_region_size();
            let len = u32::from_le_bytes(program_table_data[region_offset + 2 * mem::size_of::<u32>()..region_offset + 3 * mem::size_of::<u32>()].try_into().unwrap());
            if len & Region::ENABLE_MASK != 0 && len & Region::PHYSICAL_MASK != 0 && len & Region::DEVICE_MASK == 0 {
                let crc_offset = region_offset + 4 * mem::size_of::<u32>();
                let phys_addr = u32::from_le_bytes(program_table_data[region_offset..region_offset + mem::size_of::<u32>()].try_into().unwrap()) as usize;
                let actual_len = u32::from_le_bytes(program_table_data[region_offset + 3 * mem::size_of::<u32>()..region_offset + 4 * mem::size_of::<u32>()].try_into().unwrap()) as usize;
                let data_offset = get_file_offset_from_phys_addr(&file, phys_addr, actual_len).unwrap();
                let data = &file.data()[data_offset..data_offset + actual_len];
                let mut crc_data = Vec::new();
                for i in 0..(data.len() / 4) {
                    let i = i * 4;
                    let word = (data[i] as u32) | ((data[i + 1] as u32) << 8) | ((data[i + 2] as u32) << 16) | ((data[i + 3] as u32) << 24);
                    crc_data.push(word);
                }
                let mul = data.len() & !3;
                if mul != data.len() {
                    let last_bytes = data.len() - mul;
                    let mut final_word = 0;
                    for i in mul..data.len() {
                        final_word |= (data[i] as u32) << ((i - mul) * 8)
                    }
                    // add 0xff for final unwritten flash memory
                    for i in last_bytes..4 {
                        final_word |= 0xff << (i * 8);
                    }
                    crc_data.push(final_word);
                }
                let crc = calc_crc(&crc_data);
                update_table.push((program_table.elf_section_header().sh_offset(Endianness::Little) as usize + crc_offset, crc));
            }
        }
    }
    for (index, crc) in update_table {
        let crc = crc.to_le_bytes();
        for (i, byte) in crc.iter().enumerate() {
            data[index + i] = *byte;
        }
    }
    let mut file = File::create(&outfile).map_err(|_| 
        PkgError::WriteError {
            file: outfile.to_string()
        }
    )?;
    file.write_all(&data).map_err(|_| 
        PkgError::WriteError {
            file: outfile.to_string()
        }
    )?;
    Ok(())
}

pub fn get_file_regions(
    name: &str,
    file: &LoadedConfig, 
    allocs: &mut VecDeque<Alloc>, 
    renames: &mut HashMap<String, (Vec<SectionRename>, Vec<String>)>, 
    codes_offsets: &mut HashMap<String, usize>,
    mut codes_size: usize,
    ) -> Result<usize, PkgError> {
    
    // check relocatable
    if file.data.kind() != ObjectKind::Relocatable {
        return Err(
            PkgError::NonRelocatable {
                file: file.filename.to_string() 
            }
        );
    }
    let alloc_type = AllocType::new(name);
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
            let size = sec.sh_size(Endianness::Little) as usize;
            let addr = sec.sh_addr(Endianness::Little) as usize;
            // get section flags
            let mut flags = SectionAttr::new(true, false, false);
            if sec.sh_flags(Endianness::Little) & SHF_EXECINSTR != 0 {
                flags.set_exec(true);
            }
            if sec.sh_flags(Endianness::Little) & SHF_WRITE != 0 {
                flags.set_write(true);
            }
            let load = flags.write() || name == "kernel" && region_name == ".text.flash";

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
            let actual_size = size;
            let size = if name == "kernel" { 
                size 
            } else { 
                size.next_power_of_two().max(MIN_REGION_SIZE as usize) 
            };
            let attr = RegionAttr::try_from(flags).map_err(|err| 
                PkgError::InvalidRegionPermissions {
                    name: name.to_string(), 
                    region: region_name.to_string(), 
                    flags: err
                }
            )?;
            let alignment = if name == "kernel" || load {
                4
            } else {
                size
            };
            if alloc_type != AllocType::Kernel {
                // if store (not .bss), have codes for flash
                // if load have codes for in RAM
                if load {
                    let codes_name = format!("{}{}", name, region_name);
                    codes_offsets.insert(codes_name.to_string(), codes_size);
                    let block_len = file.block_len as usize;
                    // dividing by size of u32 has no remainder as already made sure actual size
                    // multiple of u32
                    let blocks = (actual_size / mem::size_of::<u32>()) / block_len;
                    // +1 for CRC
                    codes_size += blocks * (calc_symbol_len(block_len) + 1) * mem::size_of::<u32>();
                    let rem_block = (actual_size / mem::size_of::<u32>()) - blocks * block_len;
                    if rem_block != 0 {
                        codes_size += (calc_symbol_len(rem_block) + 1) * mem::size_of::<u32>();
                    }
                }
            }
            // put section to be allocated later
            let alloc = Alloc {
                name: name.to_string(),
                region: region_name.to_string(),
                queue: false,
                store: region_name != ".bss",
                attr,
                load,
                entry_addr,
                size,
                actual_size,
                alignment,
            };
            let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
            allocs.insert(index, alloc);
        }
    }

    if let Some(stack) = file.data.symbol_by_name("__stack_size") {
        // if have reserved a stack, allocate this as well
        let actual_stack_size = round_word(stack.address() as usize);
        let stack_size = if name == "kernel" {
            actual_stack_size
        } else {
            actual_stack_size.next_power_of_two().max(MIN_REGION_SIZE as usize)
        };
        let stack_alignment = if name == "kernel" {
            4
        } else {
            stack_size
        };
        if alloc_type != AllocType::Kernel {
            let codes_name = format!("{}.stack", name);
            codes_offsets.insert(codes_name.to_string(), codes_size);
            let block_len = file.block_len as usize;
            // dividing by size of u32 has no remainder as already made sure actual size
            // multiple of u32
            let blocks = (actual_stack_size / mem::size_of::<u32>()) / block_len;
            // +1 for CRC
            codes_size += blocks * (calc_symbol_len(block_len) + 1) * mem::size_of::<u32>();
            let rem_block = (actual_stack_size / mem::size_of::<u32>()) - blocks * block_len;
            if rem_block != 0 {
                codes_size += (calc_symbol_len(rem_block) + 1) * mem::size_of::<u32>();
            }
        }
        let alloc = Alloc { 
            name: name.to_string(),
            region: ".stack".to_string(), 
            queue: false,
            store: false,
            attr: RegionAttr::RW, 
            load: true, 
            entry_addr: None,
            size: stack_size, 
            actual_size: actual_stack_size,
            alignment: stack_alignment,
        };
        let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
        allocs.insert(index, alloc);
    }
    // add section renames to hash map
    _ = renames.insert(file.filename.to_string(), (file_secs, file_symbols));
    Ok(codes_size)
}
