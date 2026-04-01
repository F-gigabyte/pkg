use std::sync::atomic::{AtomicU32, Ordering};

use capitalize::Capitalize;

use crate::{drivers::lookup_driver, errors::PkgError, region::Region};

#[derive(Debug)]
pub struct Program {
    pub name: String,
    pub pid: u32,
    pub priority: u8,
    pub driver: u16,
    pub inter: [u8; 4],
    pub sp: Option<u32>,
    pub entry: Option<u32>,
    pub regions: [Region; 8],
    pub num_sync_queues: u8,
    pub num_sync_endpoints: u32,
    pub sync_queues: u32,
    pub sync_endpoints: u32,
    pub num_async_queues: u8,
    pub num_async_endpoints: u32,
    pub async_queues: u32,
    pub async_endpoints: u32,
    pub num_notifiers: u8,
    pub notifiers: u32,
    pub block_len: u32,
    pub pin_mask: u32
}

static PID: AtomicU32 = AtomicU32::new(0);

impl Program {
    const PRIORITY_SHIFT: usize = 0;
    const DRIVER_SHIFT: usize = 16;

    const PRIORITY_MASK: u32 = 0xff << Self::PRIORITY_SHIFT;
    const DRIVER_MASK: u32 = 0xffff << Self::DRIVER_SHIFT;
    const INTERRUPT_NONE: u8 = 0xff;

    const SYNC_QUEUES_SHIFT: usize = 0;
    const ASYNC_QUEUES_SHIFT: usize = 8;
    const NOTIFIER_QUEUES_SHIFT: usize = 16;

    const SYNC_QUEUES_MASK: u32 = 0xff << Self::SYNC_QUEUES_SHIFT;
    const ASYNC_QUEUES_MASK: u32 = 0xff << Self::ASYNC_QUEUES_SHIFT;
    const NOTIFIER_QUEUES_MASK: u32 = 0xff << Self::NOTIFIER_QUEUES_SHIFT;

    pub fn new(
        name: String,
        priority: u8,
        driver: u16,
        inter: [u8; 4],
        num_sync_queues: u8,
        num_sync_endpoints: u32,
        num_async_queues: u8,
        num_async_endpoints: u32,
        num_notifiers: u8,
        regions: [Region; 8],
        block_len: u32,
        pin_mask: u32
    ) -> Self {

        Self { 
            pid: PID.fetch_add(1, Ordering::Acquire),
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
            num_notifiers,
            notifiers: 0,
            block_len,
            pin_mask 
        }
    }

    pub fn is_reserved_name(name: &str) -> bool {
        matches!(name, "kernel" | "sync" | "async" | "program_table" | "procs" | "" | "codes" | "args")
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
        Region::get_region_size() * 8 + 15 * std::mem::size_of::<u32>()
    }

    pub fn serialise(&self) -> Result<Vec<u8>, PkgError> {
        let mut res = Vec::new();
        res.extend_from_slice(&(self.pid.to_le_bytes()));
        res.extend_from_slice(&(((self.priority as u32) << Self::PRIORITY_SHIFT) | ((self.driver as u32) << Self::DRIVER_SHIFT)).to_le_bytes());
        res.extend_from_slice(&self.inter);
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
        let num_queues = ((self.num_sync_queues as u32) << Self::SYNC_QUEUES_SHIFT) | ((self.num_async_queues as u32) << Self::ASYNC_QUEUES_SHIFT) | ((self.num_notifiers as u32) << Self::NOTIFIER_QUEUES_SHIFT);
        res.extend_from_slice(&num_queues.to_le_bytes());
        res.extend_from_slice(&self.num_sync_endpoints.to_le_bytes());
        res.extend_from_slice(&self.sync_queues.to_le_bytes());
        res.extend_from_slice(&self.sync_endpoints.to_le_bytes());
        res.extend_from_slice(&self.num_async_endpoints.to_le_bytes());
        res.extend_from_slice(&self.async_queues.to_le_bytes());
        res.extend_from_slice(&self.async_endpoints.to_le_bytes());
        res.extend_from_slice(&self.notifiers.to_le_bytes());
        res.extend_from_slice(&self.block_len.to_le_bytes());
        res.extend_from_slice(&self.pin_mask.to_le_bytes());
        Ok(res)
    }

    pub fn display(&self, indent: usize) {
        let indent_len = indent;
        let indent = "\t".repeat(indent);
        println!("{}{}", indent, self.name.capitalize());
        println!("{}\tPID: {}", indent, self.pid);
        println!("{}\tPriority: {}", indent, self.priority);
        if self.driver != 0 {
            println!("{}\tDriver: {} ({})", indent, self.driver, lookup_driver(self.driver).unwrap());
        }
        for (i, inter) in self.inter.iter().enumerate() {
            if *inter != 0xff {
                println!("{}\tInterrupt {}: {}", indent, i, inter);
            }
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
        println!("{}\tNum. Notifiers: {}", indent, self.num_notifiers);
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
        if self.num_notifiers > 0 {
            println!("{}\tNotifier Address: 0x{:x}", indent, self.notifiers);
        }
        println!("{}\tBlock Length: {}", indent, self.block_len);
        if self.pin_mask != 0 {
            println!("{}\tPin Mask: {:b}", indent, self.pin_mask);
        }
    }
}
