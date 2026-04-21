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

use std::sync::atomic::{AtomicU32, Ordering};

use capitalize::Capitalize;

use crate::{devices::lookup_device, errors::PkgError, region::Region};

/// Represents a program that will be in the program table
#[derive(Debug)]
pub struct Program {
    /// The program's name
    pub name: String,
    /// The program's PID
    pub pid: u32,
    /// The program's priority
    pub priority: u8,
    /// The program's device
    pub device: u16,
    /// The program's interrupts
    pub inter: [u8; 4],
    /// The program's stack region
    pub sp: Option<u32>,
    /// The program's entry point
    pub entry: Option<u32>,
    /// The program's regions
    pub regions: [Region; 8],
    /// The number of synchronous queues this program has
    pub num_sync_queues: u8,
    /// The number of synchronous endpoints this program has
    pub num_sync_endpoints: u32,
    /// The address of this program's synchronous queues
    pub sync_queues: u32,
    /// The address of this program's synchronous endpoints
    pub sync_endpoints: u32,
    /// The number of asynchronous queues this program has
    pub num_async_queues: u8,
    /// The number of asynchronous endpoints this program has
    pub num_async_endpoints: u32,
    /// The address of this program's asynchronous queues
    pub async_queues: u32,
    /// The address of this program's asynchronous endpoints
    pub async_endpoints: u32,
    /// The number of notifier queues this program has
    pub num_notifiers: u8,
    /// The address of this program's notifier queues
    pub notifiers: u32,
    /// The program's hamming block length
    pub block_len: u32,
    /// The program's allocated pins mask
    pub pin_mask: u32,
    // not serialised but has end of stack address placed here
    /// The address of the program's stack arguments if present
    pub stack_args_addr: Option<usize>,
}

/// Next PID to allocate
static PID: AtomicU32 = AtomicU32::new(0);

impl Program {
    /// Priority shift
    const PRIORITY_SHIFT: usize = 0;
    /// Device shift
    const DEVICE_SHIFT: usize = 16;

    /// Priority mask
    const PRIORITY_MASK: u32 = 0xff << Self::PRIORITY_SHIFT;
    /// Device mask
    const DEVICE_MASK: u32 = 0xffff << Self::DEVICE_SHIFT;
    /// No interrupt
    const INTERRUPT_NONE: u8 = 0xff;

    /// Number of synchronous queues shift
    const SYNC_QUEUES_SHIFT: usize = 0;
    /// Number of asynchronous queues shift
    const ASYNC_QUEUES_SHIFT: usize = 8;
    /// Number of notifier queues shift
    const NOTIFIER_QUEUES_SHIFT: usize = 16;

    /// Number of synchronous queues mask
    const SYNC_QUEUES_MASK: u32 = 0xff << Self::SYNC_QUEUES_SHIFT;
    /// Number of asynchronous queues mask
    const ASYNC_QUEUES_MASK: u32 = 0xff << Self::ASYNC_QUEUES_SHIFT;
    /// Number of notifier queues mask
    const NOTIFIER_QUEUES_MASK: u32 = 0xff << Self::NOTIFIER_QUEUES_SHIFT;

    /// Creates a new `Program`  
    /// `name` is the program name  
    /// `priority` is the program's priority  
    /// `device` is the program's device  
    /// `inter` is a list of the program's interrupts  
    /// `num_sync_queues` is the number of synchronous queues this program has  
    /// `num_sync_endpoints` is the number of synchronous endpoints this program has  
    /// `num_async_queues` is the number of asynchronous queues this program has  
    /// `num_async_endpoints` is the number of asynchronous endpoints this program has  
    /// `num_notifiers` is the number of notifier queues this program has  
    /// `regions` is a list of all the memory regions this program has  
    /// `block_len` is the program's hamming block length  
    /// `pin_mask` is a bit mask of all the pins this program has been allocated  
    /// `stack_args_addr` is the address of this program's stack arguments if present
    pub fn new(
        name: String,
        priority: u8,
        device: u16,
        inter: [u8; 4],
        num_sync_queues: u8,
        num_sync_endpoints: u32,
        num_async_queues: u8,
        num_async_endpoints: u32,
        num_notifiers: u8,
        regions: [Region; 8],
        block_len: u32,
        pin_mask: u32,
        stack_args_addr: Option<usize>
    ) -> Self {

        Self { 
            pid: PID.fetch_add(1, Ordering::Acquire),
            name, 
            priority, 
            device, 
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
            pin_mask, 
            stack_args_addr
        }
    }

    /// Checks if a name is reserved  
    /// `name` is the name to check
    pub fn is_reserved_name(name: &str) -> bool {
        matches!(name, "kernel" | "sync" | "async" | "program_table" | "procs" | "" | "codes" | "args")
    }

    /// Locates an unallocated region if possible  
    /// Returns the region index and a mutable reference to the region if found
    pub fn find_empty_region(&mut self) -> Option<(usize, &mut Region)> {
        for (i, region) in self.regions.iter_mut().enumerate() {
            if !region.is_enabled() {
                return Some((i, region));
            }
        }
        None
    }

    /// Returns the size of a program in bytes
    pub const fn get_prog_size() -> usize {
        Region::get_region_size() * 8 + 15 * std::mem::size_of::<u32>()
    }

    /// Converts the program into a byte stream
    pub fn serialise(&self) -> Result<Vec<u8>, PkgError> {
        let mut res = Vec::new();
        res.extend_from_slice(&(self.pid.to_le_bytes()));
        res.extend_from_slice(&(((self.priority as u32) << Self::PRIORITY_SHIFT) | ((self.device as u32) << Self::DEVICE_SHIFT)).to_le_bytes());
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

    /// Displays the program in a human readable form  
    /// `indent` is the number of indentations to use on top of other indentation
    pub fn display(&self, indent: usize) {
        let indent_len = indent;
        let indent = "\t".repeat(indent);
        println!("{}{}", indent, self.name.capitalize());
        println!("{}\tPID: {}", indent, self.pid);
        println!("{}\tPriority: {}", indent, self.priority);
        if self.device != 0 {
            println!("{}\tDevice: {} ({})", indent, self.device, lookup_device(self.device).unwrap());
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
