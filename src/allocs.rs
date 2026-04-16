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

use std::{cmp::Ordering, collections::{HashMap, VecDeque}};

use crate::{Section, driver_args::DriverArgs, errors::PkgError, program::Program, region::Region, region_attr::RegionAttr};

/// Kernel bootloader address
const BOOTLOADER_ADDR: usize = 0x10000000;
/// Kernel vector table offset address
const VECTORS_ADDR: usize = 0x10000000 + 256;

/// Represents how a particular memory storage has been allocated
pub struct MemMap {
    /// Name of the storage
    name: &'static str,
    /// The different allocations in the storage
    regions: Vec<Block>
}

impl MemMap {
    /// Creates a new `MemMap`  
    /// `name` is the name of the storage  
    /// `start` is the start address of the storage  
    /// `len` is how large the storage is
    pub fn new(name: &'static str, start: usize, len: usize) -> Self {
        let regions = vec![
            Block {
                lower: start,
                upper: start + len,
                region: "free".to_string()
            }
        ];
        Self { 
            name, 
            regions 
        }
    }

    /// Allocates a region of memory  
    /// The size of the allocation is the `size` attribute of `alloc`  
    /// Uses first fit as the allocation strategy  
    /// `alloc` is the allocation requirements  
    /// On success returns the address allocated while on error a `PkgError` is returned
    fn allocate(&mut self, alloc: &Alloc) -> Result<usize, PkgError> {
        for i in 0..self.regions.len() {
            if self.regions[i].region == "free" {
                let lower = (self.regions[i].lower + alloc.alignment - 1) & !(alloc.alignment - 1);
                if lower < self.regions[i].upper {
                    let size = self.regions[i].upper - lower;
                    let mut i = i;
                    if size >= alloc.size {
                        let upper = lower + alloc.size;
                        if lower != self.regions[i].lower {
                            self.regions.insert(
                                i, 
                                Block {
                                    lower: self.regions[i].lower,
                                    upper: lower,
                                    region: "free".to_string()
                                }
                            );
                            i += 1;
                        }
                        if upper != self.regions[i].upper {
                            self.regions.insert(
                                i + 1, 
                                Block {
                                    lower: upper,
                                    upper: self.regions[i].upper,
                                    region: "free".to_string()
                                }
                            );
                        }
                        self.regions[i] = Block {
                            lower,
                            upper,
                            region: format!("{}{} ({})", alloc.name, alloc.region, alloc.attr)
                        };
                        return Ok(lower);
                    }
                }
            }
        }
        Err(
            PkgError::NoSpace {
                name: alloc.name.clone(),
                region: alloc.region.clone() 
            }
        )
    }

    /// Reserves a section of memory, preventing it from being allocated  
    /// `block` is the block of memory to reserves  
    /// On failure, returns the block back
    fn reserve(&mut self, block: Block) -> Result<(), Block> {
        for i in 0..self.regions.len() {
            if self.regions[i].lower <= block.lower && self.regions[i].upper >= block.upper && self.regions[i].region == "free" {
                let mut i = i;
                if block.lower - self.regions[i].lower != 0 {
                    self.regions.insert(
                        i, 
                        Block { 
                            lower: self.regions[i].lower, 
                            upper: block.lower, 
                            region: "free".to_string() 
                        }
                    );
                    i += 1;
                }
                if self.regions[i].upper - block.upper != 0 {
                    self.regions.insert(
                        i + 1, 
                        Block { 
                            lower: block.upper, 
                            upper: self.regions[i].upper, 
                            region: "free".to_string() 
                        }
                    );
                }
                self.regions[i] = block;
                return Ok(());
            }
        }
        Err(block)
    }

    /// Prints the memory map in a human readable format  
    /// `indent` is the base indentation level to add on top of any other indentation
    pub fn display(&self, indent: usize) {
        let indent = "\t".repeat(indent).to_string();
        println!("{}{}", indent, self.name);
        for block in &self.regions {
            println!("{}\t0x{:x} -> 0x{:x}: {}", indent, block.lower, block.upper, block.region);
        }
    }
}

/// A block of memory
#[derive(Debug)]
struct Block {
    /// Memory start address
    lower: usize,
    /// Memory end address
    upper: usize,
    /// Region's name
    region: String
}

/// An allocation requirement
#[derive(Debug, Clone, Eq)]
pub struct Alloc {
    /// Name of the program allocating
    pub name: String,
    /// Name of the program region being allocated
    pub region: String,
    /// Do not add a link section for this allocation
    pub no_section: bool,
    /// Region access modifiers
    pub attr: RegionAttr,
    /// Whether this region should be loaded into RAM
    pub load: bool,
    /// Whether this region should also exist in flash
    pub store: bool,
    /// Region entry address if present
    pub entry_addr: Option<usize>,
    /// Region size padded to MPU requirements if needed
    pub size: usize,
    /// Region's actual size without padding
    pub actual_size: usize,
    /// Region aligment requirements
    pub alignment: usize,
    /// Region stack arguments address if present
    pub stack_args: Option<usize>
}

/// Orders allocations  
/// Puts the bootloader first, followed by the vector table, followed by the rest of the kernel
/// sections ordered from highest alignment, highest size to lowest alignment, lowest size and then
/// program regions from highest alignment, highest size to lowest alignment, lowest size
impl PartialOrd for Alloc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Orders allocations  
/// Puts the bootloader first, followed by the vector table, followed by the rest of the kernel
/// sections ordered from highest alignment, highest size to lowest alignment, lowest size and then
/// program regions from highest alignment, highest size to lowest alignment, lowest size
impl Ord for Alloc {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.name == "kernel", other.name == "kernel") {
            (true, true) => {
                if self.region == ".bootloader" {
                    Ordering::Less
                } else if other.region == ".bootloader" {
                    Ordering::Greater
                } else if self.region == ".text.vectors" {
                    Ordering::Less
                } else if self.region == ".text.vectors" {
                    Ordering::Greater
                } else if self.alignment == other.alignment { 
                    if self.size == other.size {
                        Ordering::Equal
                    } else if self.size > other.size {
                        Ordering::Less
                    } else {
                        Ordering::Greater 
                    }
                } else if self.alignment > other.alignment { 
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
                if self.alignment == other.alignment { 
                    if self.size == other.size {
                        Ordering::Equal
                    } else if self.size > other.size {
                        Ordering::Less
                    } else {
                        Ordering::Greater 
                    }
                } else if self.alignment > other.alignment { 
                    Ordering::Less 
                } else { 
                    Ordering::Greater 
                }
            }
        }
    }
}

/// Tests if two allocations are equal
impl PartialEq for Alloc {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

/// Represents allocation information as it's being generated
struct PartialAllocInfo {
    /// The kernel entry address
    kernel_entry: Option<usize>,
    /// The kernel stack
    kernel_stack: Option<usize>,
    /// The program table physical address
    prog_table_phys: Option<usize>,
    /// The synchronous queues virtual address
    sync_queues_virt: Option<usize>,
    /// The synchronous queues length in bytes
    sync_queues_len: Option<usize>,
    /// The asynchronous queues physical address
    async_queues_phys: Option<usize>,
    /// The asynchronous queues virtual address
    async_queues_virt: Option<usize>,
    /// The asynchronous queues length in bytes
    async_queues_len: Option<usize>,
    /// The asynchronous messages virtual address
    messages_virt: Option<usize>,
    /// The messages length in bytes
    messages_len: Option<usize>,
    /// The synchronous endpoints physical address
    sync_endpoints_phys: Option<usize>,
    /// The asynchronous endpoints physical address
    async_endpoints_phys: Option<usize>,
    /// The notifier queues virtual address
    notifier_virt: Option<usize>,
    /// The notifier queues length in bytes
    notifier_len: Option<usize>,
    /// The process table virtual address
    proc_virt: Option<usize>,
    /// The process table length in bytes
    proc_len: Option<usize>,
    /// The hamming codes virtual address
    codes: Option<usize>,
    /// The kernel arguments physial address
    args: Option<usize>
}

impl PartialAllocInfo {
    /// Creates a new `PartialAllocInfo`  
    /// Everythings initialised to `None`
    pub fn new() -> Self {
        Self { 
            kernel_entry: None, 
            kernel_stack: None, 
            prog_table_phys: None, 
            sync_queues_virt: None, 
            sync_queues_len: None, 
            async_queues_phys: None, 
            async_queues_virt: None, 
            async_queues_len: None, 
            messages_virt: None, 
            messages_len: None, 
            sync_endpoints_phys: None, 
            async_endpoints_phys: None, 
            notifier_virt: None,
            notifier_len: None,
            proc_virt: None, 
            proc_len: None,
            codes: None,
            args: None,
        }
    }

    /// Finalises the allocation information  
    /// If the `kernel_entry` or `kernel_stack` are not present, a `PkgError` is returned  
    /// Panics if any other information is missing  
    /// Returns an `AllocInfo` on success
    pub fn finalise(self) -> Result<AllocInfo, PkgError> {
        Ok(AllocInfo {
            kernel_entry: self.kernel_entry.ok_or(PkgError::NoKernelEntry)?,
            kernel_stack: self.kernel_stack.ok_or(PkgError::NoKernelStack)?,
            prog_table_phys: self.prog_table_phys.unwrap(),
            sync_queues_virt: self.sync_queues_virt.unwrap(),
            sync_queues_len: self.sync_queues_len.unwrap(),
            async_queues_phys: self.async_queues_phys.unwrap(),
            async_queues_virt: self.async_queues_virt.unwrap(),
            async_queues_len: self.async_queues_len.unwrap(),
            messages_virt: self.messages_virt.unwrap(),
            messages_len: self.messages_len.unwrap(),
            sync_endpoints_phys: self.sync_endpoints_phys.unwrap(),
            async_endpoints_phys: self.async_endpoints_phys.unwrap(),
            notifier_virt: self.notifier_virt.unwrap(),
            notifier_len: self.notifier_len.unwrap(),
            proc_virt: self.proc_virt.unwrap(),
            proc_len: self.proc_len.unwrap(),
            codes: self.codes.unwrap(),
            args: self.args.unwrap()
        })
    }
}

/// Allocation information to be passed to the rest of the program
pub struct AllocInfo {
    /// The kernel entry address
    pub kernel_entry: usize,
    /// The kernel stack
    pub kernel_stack: usize,
    /// The program table physical address
    pub prog_table_phys: usize,
    /// The synchronous queues virtual address
    pub sync_queues_virt: usize,
    /// The synchronous queues length in bytes
    pub sync_queues_len: usize,
    /// The asynchronous queues physical address
    pub async_queues_phys: usize,
    /// The asynchronous queues virtual address
    pub async_queues_virt: usize,
    /// The asynchronous queues length in bytes
    pub async_queues_len: usize,
    /// The asynchronous messages virtual address
    pub messages_virt: usize,
    /// The messages length in bytes
    pub messages_len: usize,
    /// The synchronous endpoints physical address
    pub sync_endpoints_phys: usize,
    /// The asynchronous endpoints physical address
    pub async_endpoints_phys: usize,
    /// The notifier queues virtual address
    pub notifier_virt: usize,
    /// The notifier queues length in bytes
    pub notifier_len: usize,
    /// The process table virtual address
    pub proc_virt: usize,
    /// The process table length in bytes
    pub proc_len: usize,
    /// The hamming codes virtual address
    pub codes: usize,
    /// The kernel arguments physial address
    pub args: usize
}

/// The allocation type
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AllocType {
    /// A kernel section is being allocated
    Kernel,
    /// A synchronous queue, notifier or endpoint is being allocated
    Sync,
    /// An asynchronous queue, endpoint or message is being allocated
    Async,
    /// The program table is being allocated
    ProgramTable,
    /// The process table is being allocated
    Procs,
    /// The hamming codes are being allocated
    Codes,
    /// The kernel arguments are being allocated
    Args,
    /// A normal region is being allocated
    Other
}

/// Determines the type of allocation being made  
/// `name` is the name of the program allocating
impl AllocType {
    pub fn new(name: &str) -> Self {
        match name {
            "kernel" => Self::Kernel,
            "sync" => Self::Sync,
            "async" => Self::Async,
            "program_table" => Self::ProgramTable,
            "procs" => Self::Procs,
            "args" => Self::Args,
            "codes" => Self::Codes,
            _ => Self::Other
        }
    }
}

/// Performs all the allocations  
/// `allocs` is a sorted list of allocations to make  
/// `ram` is the RAM memory map  
/// `flash` is the flash memory map  
/// `sections` is a list of sections that will end up in the final elf file  
/// `programs` is a list of program information for use in the program table  
/// Returns the allocation information on success or a `PkgError` on failure
pub fn do_allocs(
    allocs: VecDeque<Alloc>, 
    ram: &mut MemMap, 
    flash: &mut MemMap, 
    sections: &mut Vec<Section>, 
    programs: &mut HashMap<String, Program>,
    ) -> Result<AllocInfo, PkgError> {
    let mut alloc_info = PartialAllocInfo::new();
    for mut alloc in allocs {
        let alloc_type = AllocType::new(&alloc.name);
        let mut virt_addr = None;
        let name = format!("{}{}", alloc.name, alloc.region);
        let entry_addr = alloc.entry_addr.take();
        if alloc.load {
            virt_addr = Some(ram.allocate(&alloc)?);
        }
        let mut phys_addr = None;
        if alloc.store {
            if alloc_type == AllocType::Kernel && alloc.region == ".bootloader" {
                let block = Block {
                    lower: BOOTLOADER_ADDR, 
                    upper: BOOTLOADER_ADDR + alloc.size, 
                    region: format!("{}{} ({})", alloc.name, alloc.region, alloc.attr)
                };
                // not using closure as rust can't tell value moved in gets returned before it is
                // used elsewhere in the program
                if let Err(_) = flash.reserve(block) { 
                    return Err(PkgError::NoSpace { 
                        name: alloc.name, 
                        region: alloc.region
                    });
                }
                phys_addr = Some(BOOTLOADER_ADDR);
            } else if alloc_type == AllocType::Kernel && alloc.region == ".text.vectors" {
                let block = Block {
                    lower: VECTORS_ADDR, 
                    upper: VECTORS_ADDR + alloc.size, 
                    region: format!("{}{} ({})", alloc.name, alloc.region, alloc.attr)
                };
                if let Err(_) = flash.reserve(block) { 
                    return Err(PkgError::NoSpace { 
                        name: alloc.name, 
                        region: alloc.region 
                    });
                }
                phys_addr = Some(VECTORS_ADDR);
            } else {
                if alloc.load {
                    // if loading section, don't need to align physical address to size boundary
                    alloc.alignment = 4;
                }
                phys_addr = Some(flash.allocate(&alloc)?); 
            }
        }
        if virt_addr.is_none() && phys_addr.is_none() {
            continue;
        }
        let runtime_addr = virt_addr.unwrap_or_else(|| phys_addr.unwrap());
        if alloc.region != ".stack" && !alloc.no_section && alloc_type != AllocType::ProgramTable {
            let virt_addr = virt_addr.unwrap_or_else(|| phys_addr.unwrap());
            let phys_addr = phys_addr.unwrap_or(virt_addr);
            let alloc_sec = Section {
                name,
                phys_addr,
                virt_addr,
            };
            sections.push(alloc_sec);
        }
        match alloc_type {
            AllocType::Other => {
                if let Some(program) = programs.get_mut(&alloc.name) {
                    let (index, sec) = program.find_empty_region().ok_or(
                        PkgError::TooManySections {
                            name: alloc.name.to_string()
                        }
                    )?;
                    sec.name = Some(alloc.region.to_string());
                    let zero = if alloc.region == ".bss" {
                        Region::ZERO_MASK
                    } else {
                        0
                    };
                    let mut region_type = 0;
                    if phys_addr.is_some() {
                        region_type |= Region::PHYSICAL_MASK;
                    }
                    if virt_addr.is_some() {
                        region_type |= Region::VIRTUAL_MASK;
                    }
                    sec.len = ((alloc.size.ilog2() - 1) as u32) << Region::LEN_SHIFT | 
                        ((alloc.attr as u32) << Region::PERM_SHIFT) | 
                        Region::ENABLE_MASK | 
                        zero | 
                        region_type;
                    sec.actual_len = alloc.actual_size as u32;
                    sec.phys_addr = phys_addr.unwrap_or(0) as u32;
                    sec.virt_addr = virt_addr.unwrap_or(0) as u32;
                    if alloc.region == ".stack" {
                        program.sp = Some(index as u32);
                    }
                    if let Some(entry_addr) = entry_addr {
                        program.entry = Some(entry_addr as u32 + runtime_addr as u32);
                    }
                    if let Some(stack_args) = alloc.stack_args {
                        // should be in .rodata section so ok
                        program.stack_args_addr = Some(stack_args + phys_addr.unwrap())
                    }
                } else {
                    return Err(
                        PkgError::NoProgram {
                            name: alloc.name
                        }
                    );
                }
            },
            AllocType::Codes => {
                alloc_info.codes = Some(runtime_addr);
            },
            AllocType::Args => {
                alloc_info.args = Some(runtime_addr);
            },
            AllocType::Kernel => {
                if let Some(entry_addr) = entry_addr {
                    alloc_info.kernel_entry = Some(entry_addr + runtime_addr);
                }
                if alloc.region == ".stack" {
                    alloc_info.kernel_stack = Some(runtime_addr + alloc.size);
                }
            },
            AllocType::ProgramTable => {
                alloc_info.prog_table_phys = Some(runtime_addr)
            },
            AllocType::Sync => {
                match alloc.region.as_ref() {
                    ".queues" => {
                        alloc_info.sync_queues_virt = Some(runtime_addr);
                        alloc_info.sync_queues_len = Some(alloc.size);
                    }
                    ".endpoints" => alloc_info.sync_endpoints_phys = Some(runtime_addr),
                    ".notifier" => {
                        alloc_info.notifier_virt = Some(runtime_addr);
                        alloc_info.notifier_len = Some(alloc.size);
                    },
                    _ => {}
                }
            },
            AllocType::Async => {
                match alloc.region.as_ref() {
                    ".queues" => {
                        alloc_info.async_queues_virt = Some(virt_addr.unwrap());
                        alloc_info.async_queues_phys = Some(phys_addr.unwrap());
                        alloc_info.async_queues_len = Some(alloc.size);
                    },
                    ".endpoints" => alloc_info.async_endpoints_phys = Some(runtime_addr),
                    ".messages" => {
                        alloc_info.messages_virt = Some(runtime_addr);
                        alloc_info.messages_len = Some(alloc.size);
                    }
                    _ => {}
                }
            },
            AllocType::Procs => {
                alloc_info.proc_virt = Some(runtime_addr);
                alloc_info.proc_len = Some(alloc.size);
            }
        }
    }
    Ok(alloc_info.finalise()?)
}

/// Generates a list of default allocations needed ontop of the kernel and program allocations  
/// `prog_table_size` is the size of the program table  
/// `procs_size` is the size of the process table  
/// `sync_queues_size` is the size of the synchronous queues in bytes  
/// `async_queues_size` is the size of the asynchronous queues in bytes  
/// `sync_endpoints_size` is the size of the synchronous endpoints in bytes  
/// `async_endpoints_size` is the size of the asynchronous endpoints in bytes  
/// `messages_size` is the size of the asynchronous messages in bytes  
/// `notifier_size` is the size of the notifier queues in bytes
/// Returns a sorted list of allocations
pub fn default_allocs(
    prog_table_size: usize, 
    procs_size: usize,
    sync_queues_size: usize,
    async_queues_size: usize,
    sync_endpoints_size: usize,
    async_endpoints_size: usize,
    messages_size: usize,
    notifier_size: usize
    ) -> VecDeque<Alloc> {
    
    let mut allocs = VecDeque::new();

    // program table alloc
    let alloc = Alloc {
        name: "program_table".to_string(),
        region: ".program_table".to_string(),
        no_section: false,
        attr: RegionAttr::R,
        load: false,
        store: true,
        entry_addr: None,
        size: prog_table_size,
        actual_size: prog_table_size,
        alignment: 4,
        stack_args: None
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);

    // processes alloc
    let alloc = Alloc {
        name: "procs".to_string(),
        region: ".procs".to_string(),
        no_section: true,
        attr: RegionAttr::RW,
        load: true,
        store: false,
        entry_addr: None,
        size: procs_size,
        actual_size: procs_size,
        alignment: 4,
        stack_args: None
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);
    
    // args alloc
    let alloc = Alloc {
        name: "args".to_string(),
        region: ".args".to_string(),
        no_section: true,
        attr: RegionAttr::R,
        load: false,
        store: true,
        entry_addr: None,
        size: std::mem::size_of::<DriverArgs>(),
        actual_size: std::mem::size_of::<DriverArgs>(),
        alignment: 4,
        stack_args: None
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);
    
    // synchronous queues alloc
    let alloc = Alloc {
        name: "sync".to_string(),
        region: ".queues".to_string(),
        no_section: true,
        attr: RegionAttr::RW,
        load: true,
        store: false,
        entry_addr: None,
        size: sync_queues_size,
        actual_size: sync_queues_size,
        alignment: 4,
        stack_args: None
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);
    
    // asynchronous queues alloc
    let alloc = Alloc {
        name: "async".to_string(),
        region: ".queues".to_string(),
        no_section: true,
        attr: RegionAttr::RW,
        load: true,
        store: true,
        entry_addr: None,
        size: async_queues_size,
        actual_size: async_queues_size,
        alignment: 4,
        stack_args: None
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);

    // synchronous endpoints alloc
    let alloc = Alloc {
        name: "sync".to_string(),
        region: ".endpoints".to_string(),
        no_section: true,
        attr: RegionAttr::R,
        load: false,
        store: true,
        entry_addr: None,
        size: sync_endpoints_size,
        actual_size: sync_endpoints_size,
        alignment: 4,
        stack_args: None
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);
    
    // asynchronous endpoints alloc
    let alloc = Alloc {
        name: "async".to_string(),
        region: ".endpoints".to_string(),
        no_section: true,
        attr: RegionAttr::R,
        load: false,
        store: true,
        entry_addr: None,
        size: async_endpoints_size,
        actual_size: async_endpoints_size,
        alignment: 4,
        stack_args: None
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);
    
    // messages alloc
    let alloc = Alloc {
        name: "async".to_string(),
        region: ".messages".to_string(),
        no_section: false,
        attr: RegionAttr::RW,
        load: true,
        store: false,
        entry_addr: None,
        size: messages_size,
        actual_size: messages_size,
        alignment: 4,
        stack_args: None
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);

    // notifiers alloc
    let alloc = Alloc {
        name: "sync".to_string(),
        region: ".notifier".to_string(),
        no_section: true,
        attr: RegionAttr::RW,
        load: true,
        store: false,
        entry_addr: None,
        size: notifier_size,
        actual_size: notifier_size,
        alignment: 4,
        stack_args: None
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);

    allocs
}

/// Adds the error codes to the list of allocations whose size isn't known at the time of the
/// default allocations  
/// `allocs` is the list of current allocations  
/// `codes_size` is the size of the hamming error codes in bytes
pub fn add_error_codes(allocs: &mut VecDeque<Alloc>, codes_size: usize) {
    // codes alloc
    let alloc = Alloc {
        name: "codes".to_string(),
        region: ".codes".to_string(),
        no_section: false,
        attr: RegionAttr::RW,
        load: true,
        store: false,
        entry_addr: None,
        size: codes_size,
        actual_size: codes_size,
        alignment: 4,
        stack_args: None
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);
}

mod test {
    use super::*;

    #[test]
    fn test_alloc_order() {
        let mut allocs = VecDeque::new();

        let alloc1 = Alloc {
            name: "alloc1".to_string(),
            region: ".alloc".to_string(),
            no_section: false,
            attr: RegionAttr::RW,
            load: true,
            store: true,
            entry_addr: None,
            size: 256,
            actual_size: 256,
            alignment: 256,
            stack_args: None
        };
        let index = allocs.binary_search(&alloc1).unwrap_or_else(|val| val);
        allocs.insert(index, alloc1);
        
        let alloc2 = Alloc {
            name: "alloc2".to_string(),
            region: ".alloc".to_string(),
            no_section: false,
            attr: RegionAttr::RW,
            load: true,
            store: true,
            entry_addr: None,
            size: 700,
            actual_size: 700,
            alignment: 4,
            stack_args: None
        };
        let index = allocs.binary_search(&alloc2).unwrap_or_else(|val| val);
        allocs.insert(index, alloc2);

        assert_eq!(allocs[0].name, "alloc1");
        assert_eq!(allocs[1].name, "alloc2");
    }

    #[test]
    fn test_alloc_type() {
        assert!(matches!(AllocType::new("kernel"), AllocType::Kernel));
        assert!(matches!(AllocType::new("sync"), AllocType::Sync));
        assert!(matches!(AllocType::new("async"), AllocType::Async));
        assert!(matches!(AllocType::new("program_table"), AllocType::ProgramTable));
        assert!(matches!(AllocType::new("procs"), AllocType::Procs));
        assert!(matches!(AllocType::new("args"), AllocType::Args));
        assert!(matches!(AllocType::new("codes"), AllocType::Codes));
        assert!(matches!(AllocType::new("uart"), AllocType::Other));
    }

    #[test]
    fn test_allocate() {
        let mut mem_map = MemMap::new("test", 0, 1000);
        let alloc = Alloc {
            name: "test_alloc".to_string(),
            region: ".test".to_string(),
            no_section: false,
            attr: RegionAttr::RW,
            load: true,
            store: true,
            entry_addr: None,
            size: 100,
            actual_size: 100,
            alignment: 4,
            stack_args: None
        };
        mem_map.reserve(Block { 
            lower: 0, 
            upper: 146, 
            region: "reserved".to_string() 
        }).unwrap();
        mem_map.reserve(Block { 
            lower: 248, 
            upper: 308, 
            region: "reserved".to_string() 
        }).unwrap();
        assert_eq!(mem_map.allocate(&alloc).unwrap(), 148);
        assert_eq!(mem_map.regions.len(), 5);
        assert_eq!(mem_map.regions[0].lower, 0);
        assert_eq!(mem_map.regions[0].upper, 146);
        assert_eq!(mem_map.regions[0].region, "reserved");
        assert_eq!(mem_map.regions[1].lower, 146);
        assert_eq!(mem_map.regions[1].upper, 148);
        assert_eq!(mem_map.regions[1].region, "free");
        assert_eq!(mem_map.regions[2].lower, 148);
        assert_eq!(mem_map.regions[2].upper, 248);
        assert_eq!(mem_map.regions[2].region, "test_alloc.test (rw)");
        assert_eq!(mem_map.regions[3].lower, 248);
        assert_eq!(mem_map.regions[3].upper, 308);
        assert_eq!(mem_map.regions[3].region, "reserved");
        assert_eq!(mem_map.regions[4].lower, 308);
        assert_eq!(mem_map.regions[4].upper, 1000);
        assert_eq!(mem_map.regions[4].region, "free");
    }
}
