use std::{cmp::Ordering, collections::{HashMap, VecDeque}};

use crate::{BOOTLOADER_ADDR, Section, VECTORS_ADDR, errors::PkgError, program::Program, region::Region, region_attr::RegionAttr};

pub struct MemMap {
    name: &'static str,
    regions: Vec<Block>
}

impl MemMap {
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

    fn allocate(&mut self, alloc: &Alloc) -> Result<usize, PkgError> {
        let mut suggest = None;
        let mut overflow = 0;
        for i in 0..self.regions.len() {
            if self.regions[i].region == "free" {
                let lower = (self.regions[i].lower + alloc.alignment - 1) & !(alloc.alignment - 1);
                if lower < self.regions[i].upper {
                    let size = self.regions[i].upper - lower;
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
            let lower = (self.regions[suggest].lower + alloc.alignment - 1) & !(alloc.alignment - 1);
            let upper = lower + alloc.size;
            if lower != self.regions[suggest].lower {
                self.regions.insert(
                    suggest, 
                    Block {
                        lower: self.regions[suggest].lower,
                        upper: lower,
                        region: "free".to_string()
                    }
                );
                suggest += 1;
            }
            if upper != self.regions[suggest].upper {
                self.regions.insert(
                    suggest + 1, 
                    Block {
                        lower: upper,
                        upper: self.regions[suggest].upper,
                        region: "free".to_string()
                    }
                );
            }
            self.regions[suggest] = Block {
                lower,
                upper,
                region: format!("{}{} ({})", alloc.name, alloc.region, alloc.attr)
            };
            Ok(lower)
        } else {
            Err(
                PkgError::NoSpace {
                    name: alloc.name.clone(),
                    region: alloc.region.clone() 
                }
            )
        }
    }

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

    pub fn display(&self, indent: usize) {
        let indent = "\t".repeat(indent).to_string();
        println!("{}{}", indent, self.name);
        for block in &self.regions {
            println!("{}\t0x{:x} -> 0x{:x}: {}", indent, block.lower, block.upper, block.region);
        }
    }
}

#[derive(Debug)]
struct Block {
    lower: usize,
    upper: usize,
    region: String
}

#[derive(Debug, Clone, Eq)]
pub struct Alloc {
    pub name: String,
    pub region: String,
    pub queue: bool,
    pub need_region: bool,
    pub attr: RegionAttr,
    pub load: bool,
    pub store: bool,
    pub entry_addr: Option<usize>,
    pub size: usize,
    pub alignment: usize
}

impl PartialOrd for Alloc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

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
                    } else if self.size < other.size {
                        Ordering::Less
                    } else {
                        Ordering::Greater 
                    }
                } else if self.alignment < other.alignment { 
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
                    } else if self.size < other.size {
                        Ordering::Less
                    } else {
                        Ordering::Greater 
                    }
                } else if self.alignment < other.alignment { 
                    Ordering::Less 
                } else { 
                    Ordering::Greater 
                }
            }
        }
    }
}

impl PartialEq for Alloc {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

struct PartialAllocInfo {
    kernel_entry: Option<usize>,
    kernel_stack: Option<usize>,
    prog_table_phys: Option<usize>,
    sync_queues_virt: Option<usize>,
    sync_queues_len: Option<usize>,
    async_queues_phys: Option<usize>,
    async_queues_virt: Option<usize>,
    async_queues_len: Option<usize>,
    messages_virt: Option<usize>,
    messages_len: Option<usize>,
    sync_endpoints_phys: Option<usize>,
    async_endpoints_phys: Option<usize>,
    proc_virt: Option<usize>,
    proc_len: Option<usize>
}

impl PartialAllocInfo {
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
            proc_virt: None, 
            proc_len: None 
        }
    }

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
            proc_virt: self.proc_virt.unwrap(),
            proc_len: self.proc_len.unwrap(),
        })
    }
}

pub struct AllocInfo {
    pub kernel_entry: usize,
    pub kernel_stack: usize,
    pub prog_table_phys: usize,
    pub sync_queues_virt: usize,
    pub sync_queues_len: usize,
    pub async_queues_phys: usize,
    pub async_queues_virt: usize,
    pub async_queues_len: usize,
    pub messages_virt: usize,
    pub messages_len: usize,
    pub sync_endpoints_phys: usize,
    pub async_endpoints_phys: usize,
    pub proc_virt: usize,
    pub proc_len: usize
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum AllocType {
    Kernel,
    Sync,
    Async,
    ProgramTable,
    Procs,
    Other
}

impl AllocType {
    pub fn new(name: &str) -> Self {
        match name {
            "kernel" => Self::Kernel,
            "sync" => Self::Sync,
            "async" => Self::Async,
            "program_table" => Self::ProgramTable,
            "procs" => Self::Procs,
            _ => Self::Other
        }
    }
}

pub fn do_allocs(
    allocs: VecDeque<Alloc>, 
    ram: &mut MemMap, 
    flash: &mut MemMap, 
    sections: &mut Vec<Section>, 
    programs: &mut HashMap<String, Program>
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
        let virt_addr = virt_addr.unwrap_or_else(|| phys_addr.unwrap());
        let phys_addr = phys_addr.unwrap_or(virt_addr);
        if alloc.region != ".stack" && !alloc.queue && alloc_type != AllocType::ProgramTable {
            let alloc_sec = Section {
                name,
                phys_addr,
                virt_addr,
            };
            sections.push(alloc_sec);
        }
        match alloc_type {
            AllocType::Other => {
                if alloc.need_region {
                    if let Some(program) = programs.get_mut(&alloc.name) {
                        let sec = program.find_empty_region().ok_or(
                            PkgError::TooManySections {
                                name: alloc.name
                            }
                        )?;
                        sec.len = alloc.size as u32;
                        sec.phys_addr = phys_addr as u32;
                        let zero = if alloc.region == ".bss" {
                            Region::ZERO_MASK
                        } else {
                            0
                        };
                        sec.virt_addr = virt_addr as u32 | ((alloc.attr as u32) << Region::PERM_SHIFT) | Region::ENABLE_MASK | zero;
                        if alloc.region == ".stack" {
                            program.sp = Some(virt_addr as u32 + alloc.size as u32);
                        }
                        if let Some(entry_addr) = entry_addr {
                            program.entry = Some(entry_addr as u32 + virt_addr as u32);
                        }
                    } else {
                        return Err(
                            PkgError::NoProgram {
                                name: alloc.name
                            }
                        );
                    }
                }
            },
            AllocType::Kernel => {
                if let Some(entry_addr) = entry_addr {
                    alloc_info.kernel_entry = Some(entry_addr + virt_addr);
                }
                if alloc.region == ".stack" {
                    alloc_info.kernel_stack = Some(virt_addr + alloc.size);
                }
            },
            AllocType::ProgramTable => {
                alloc_info.prog_table_phys = Some(phys_addr)
            },
            AllocType::Sync => {
                match alloc.region.as_ref() {
                    ".queues" => {
                        alloc_info.sync_queues_virt = Some(virt_addr);
                        alloc_info.sync_queues_len = Some(alloc.size);
                    }
                    ".endpoints" => alloc_info.sync_endpoints_phys = Some(phys_addr),
                    _ => {}
                }
            },
            AllocType::Async => {
                match alloc.region.as_ref() {
                    ".queues" => {
                        alloc_info.async_queues_virt = Some(virt_addr);
                        alloc_info.async_queues_phys = Some(phys_addr);
                        alloc_info.async_queues_len = Some(alloc.size);
                    },
                    ".endpoints" => alloc_info.async_endpoints_phys = Some(phys_addr),
                    ".messages" => {
                        alloc_info.messages_virt = Some(virt_addr);
                        alloc_info.messages_len = Some(alloc.size);
                    }
                    _ => {}
                }
            },
            AllocType::Procs => {
                alloc_info.proc_virt = Some(virt_addr);
                alloc_info.proc_len = Some(alloc.size);
            }
        }
    }
    Ok(alloc_info.finalise()?)
}

pub fn default_allocs(
    prog_table_size: usize, 
    procs_size: usize,
    sync_queues_size: usize,
    async_queues_size: usize,
    sync_endpoints_size: usize,
    async_endpoints_size: usize,
    messages_size: usize
    ) -> VecDeque<Alloc> {
    
    let mut allocs = VecDeque::new();

    // program table alloc
    let alloc = Alloc {
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
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);

    // processes alloc
    let alloc = Alloc {
        name: "procs".to_string(),
        region: ".procs".to_string(),
        queue: true,
        need_region: false,
        attr: RegionAttr::RW,
        load: true,
        store: false,
        entry_addr: None,
        size: procs_size,
        alignment: 4
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);
    
    // synchronous queues alloc
    let alloc = Alloc {
        name: "sync".to_string(),
        region: ".queues".to_string(),
        queue: true,
        need_region: false,
        attr: RegionAttr::RW,
        load: true,
        store: false,
        entry_addr: None,
        size: sync_queues_size,
        alignment: 4
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);
    
    // asynchronous queues alloc
    let alloc = Alloc {
        name: "async".to_string(),
        region: ".queues".to_string(),
        queue: true,
        need_region: false,
        attr: RegionAttr::RW,
        load: true,
        store: true,
        entry_addr: None,
        size: async_queues_size,
        alignment: 4
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);

    // synchronous endpoints alloc
    let alloc = Alloc {
        name: "sync".to_string(),
        region: ".endpoints".to_string(),
        queue: true,
        need_region: false,
        attr: RegionAttr::R,
        load: false,
        store: true,
        entry_addr: None,
        size: sync_endpoints_size,
        alignment: 4
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);
    
    // asynchronous endpoints alloc
    let alloc = Alloc {
        name: "async".to_string(),
        region: ".endpoints".to_string(),
        queue: true,
        need_region: false,
        attr: RegionAttr::R,
        load: false,
        store: true,
        entry_addr: None,
        size: async_endpoints_size,
        alignment: 4
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);
    
    // messages alloc
    let alloc = Alloc {
        name: "async".to_string(),
        region: ".messages".to_string(),
        queue: false,
        need_region: false,
        attr: RegionAttr::RW,
        load: true,
        store: false,
        entry_addr: None,
        size: messages_size,
        alignment: 4
    };
    let index = allocs.binary_search(&alloc).unwrap_or_else(|val| val);
    allocs.insert(index, alloc);

    allocs
}
