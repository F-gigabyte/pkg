use std::collections::{HashMap, VecDeque};

use object::{Endianness, Object, ObjectKind, ObjectSection, ObjectSymbol, SectionIndex, StringTable, elf::{SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, STT_FUNC}, read::elf::{FileHeader, SectionHeader}};

use crate::{allocs::Alloc, errors::PkgError, file_config::LoadedConfig, region_attr::RegionAttr, section_attr::SectionAttr, sections::SectionRename};

const MIN_REGION_SIZE: u32 = 256;

pub fn get_file_regions(name: &str, file: &LoadedConfig, allocs: &mut VecDeque<Alloc>, renames: &mut HashMap<String, (Vec<SectionRename>, Vec<String>)>) -> Result<(), PkgError> {
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
            let size = if name == "kernel" { size } else { size.next_power_of_two().max(MIN_REGION_SIZE as usize) };
            let attr = RegionAttr::try_from(flags).map_err(|err| 
                PkgError::InvalidRegionPermissions {
                    name: name.to_string(), 
                    region: region_name.to_string(), 
                    flags: err
                }
            )?;
            let alignment = if name == "kernel" {
                4
            } else {
                size
            };
            // put section to be allocated later
            let alloc = Alloc {
                name: name.to_string(),
                region: region_name.to_string(),
                queue: false,
                need_region: !is_kernel,
                store: region_name != ".bss",
                attr,
                load,
                entry_addr,
                size,
                alignment
            };
            let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
            allocs.insert(index, alloc);
        }
    }

    if let Some(stack) = file.data.symbol_by_name("__stack_size") {
        // if have reserved a stack, allocate this as well
        let stack_size = if name == "kernel" {
            stack.address() as usize
        } else {
            (stack.address() as usize).next_power_of_two().max(MIN_REGION_SIZE as usize)
        };
        let stack_alignment = if name == "kernel" {
            4
        } else {
            stack_size
        };
        let alloc = Alloc { 
            name: name.to_string(),
            region: ".stack".to_string(), 
            queue: false,
            need_region: !is_kernel,
            store: false,
            attr: RegionAttr::RW, 
            load: true, 
            entry_addr: None,
            size: stack_size, 
            alignment: stack_alignment
        };
        let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
        allocs.insert(index, alloc);
    }
    // add section renames to hash map
    _ = renames.insert(file.filename.to_string(), (file_secs, file_symbols));
    Ok(())
}
