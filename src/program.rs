use capitalize::Capitalize;

use crate::{errors::PkgError, region::Region};

#[derive(Debug)]
pub struct Program {
    pub name: String,
    pub priority: u8,
    pub driver: u16,
    pub inter: u8,
    pub sp: Option<u32>,
    pub entry: Option<u32>,
    pub regions: [Region; 8],
    pub num_sync_queues: u32,
    pub num_sync_endpoints: u32,
    pub sync_queues: u32,
    pub sync_endpoints: u32,
    pub num_async_queues: u32,
    pub num_async_endpoints: u32,
    pub async_queues: u32,
    pub async_endpoints: u32,
    pub block_len: u32
}

impl Program {
    pub fn new(
        name: String,
        priority: u8,
        driver: u16,
        inter: u8,
        num_sync_queues: u32,
        num_sync_endpoints: u32,
        num_async_queues: u32,
        num_async_endpoints: u32,
        regions: [Region; 8],
        block_len: u32
    ) -> Self {

        Self { 
            name, 
            priority, 
            driver, 
            inter, 
            sp: None, 
            entry: None, 
            regions, 
            num_sync_queues, 
            num_sync_endpoints, 
            sync_queues: 0, 
            sync_endpoints: 0, 
            num_async_queues, 
            num_async_endpoints, 
            async_queues: 0, 
            async_endpoints: 0,
            block_len 
        }
    }

    pub fn is_reserved_name(name: &str) -> bool {
        matches!(name, "kernel" | "sync" | "async" | "program_table" | "procs" | "" | "codes")
    }

    pub fn find_empty_region(&mut self) -> Option<(usize, &mut Region)> {
        for (i, region) in self.regions.iter_mut().enumerate() {
            if !region.is_enabled() {
                return Some((i, region));
            }
        }
        None
    }

    pub const fn get_prog_size() -> usize {
        Region::get_region_size() * 8 + 12 * std::mem::size_of::<u32>()
    }

    pub fn serialise(&self) -> Result<Vec<u8>, PkgError> {
        let mut res = Vec::new();
        res.extend_from_slice(&(self.priority as u32 | ((self.driver as u32) << 16) | ((self.inter as u32) << 8)).to_le_bytes());
        res.extend_from_slice(&self.sp.ok_or(
                PkgError::NoProgramStack {
                    name: self.name.to_string()
                }
            )?
            .to_le_bytes()
        );
        res.extend_from_slice(&self.entry.ok_or(
                PkgError::NoProgramEntry {
                    name: self.name.to_string()
                }
            )?
            .to_le_bytes()
        );
        for region in &self.regions {
            res.extend(region.serialise().iter());
        }
        res.extend_from_slice(&self.num_sync_queues.to_le_bytes());
        res.extend_from_slice(&self.num_sync_endpoints.to_le_bytes());
        res.extend_from_slice(&self.sync_queues.to_le_bytes());
        res.extend_from_slice(&self.sync_endpoints.to_le_bytes());
        res.extend_from_slice(&self.num_async_queues.to_le_bytes());
        res.extend_from_slice(&self.num_async_endpoints.to_le_bytes());
        res.extend_from_slice(&self.async_queues.to_le_bytes());
        res.extend_from_slice(&self.async_endpoints.to_le_bytes());
        res.extend_from_slice(&self.block_len.to_le_bytes());
        Ok(res)
    }

    pub fn display(&self, indent: usize) {
        let indent_len = indent;
        let indent = "\t".repeat(indent);
        println!("{}{}", indent, self.name.capitalize());
        println!("{}\tPriority: {}", indent, self.priority);
        if self.driver != 0 {
            println!("{}\tDriver: {}", indent, self.driver);
        }
        if self.inter != 0xff {
            println!("{}\tInterrupt: {}", indent, self.inter);
        }
        if let Some(sp) = self.sp {
            println!("{}\tStack Region: {}", indent, sp)
        }
        if let Some(entry) = self.entry {
            println!("{}\tEntry: 0x{:x}", indent, entry)
        }
        for (i, region) in self.regions.iter().enumerate() {
            if region.is_enabled() {
                println!("{}\tRegion {}", indent, i);
                region.display(indent_len + 2);
            }
        }
        println!("{}\tNum. Sync Queues: {}", indent, self.num_sync_queues);
        println!("{}\tNum. Async Queues: {}", indent, self.num_async_queues);
        println!("{}\tNum. Sync Endpoints: {}", indent, self.num_sync_endpoints);
        println!("{}\tNum. Async Endpoints: {}", indent, self.num_async_endpoints);
        if self.num_sync_queues > 0 {
            println!("{}\tSync Queues Address: 0x{:x}", indent, self.sync_queues);
        }
        if self.num_async_queues > 0 {
            println!("{}\tAsync Queues Address: 0x{:x}", indent, self.async_queues);
        }
        if self.num_sync_endpoints > 0 {
            println!("{}\tSync Endpoints Address: 0x{:x}", indent, self.sync_endpoints);
        }
        if self.num_async_endpoints > 0 {
            println!("{}\tAsync Endpoints Address: 0x{:x}", indent, self.async_endpoints);
        }
        println!("{}\tBlock Length: {}", indent, self.block_len);
    }
}
