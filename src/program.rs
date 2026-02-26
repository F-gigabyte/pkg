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
    pub async_endpoints: u32
}

impl Program {
    pub fn find_empty_region(&mut self) -> Option<&mut Region> {
        for region in self.regions.iter_mut() {
            if region.virt_addr & 1 == 0 {
                return Some(region);
            }
        }
        None
    }

    pub const fn get_prog_size() -> usize {
        Region::get_region_size() * 8 + 11 * std::mem::size_of::<u32>()
    }

    pub fn serialise(&self) -> Result<Vec<u8>, PkgError> {
        let mut res = Vec::new();
        res.extend_from_slice(&(self.priority as u32 | ((self.driver as u32) << 16) | ((self.inter as u32) << 8)).to_le_bytes());
        res.extend_from_slice(&self.sp.ok_or(PkgError::NoProgramStack(self.name.to_string()))?.to_le_bytes());
        res.extend_from_slice(&self.entry.ok_or(PkgError::NoProgramEntry(self.name.to_string()))?.to_le_bytes());
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
        Ok(res)
    }
}
