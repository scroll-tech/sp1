use super::{LookupId, MemoryLocalEvent, MemoryReadRecord, MemoryWriteRecord};
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct MemCopyEvent {
    pub lookup_id: LookupId,
    pub shard: u32,
    pub clk: u32,
    pub src_ptr: u32,
    pub dst_ptr: u32,
    pub read_records: Vec<MemoryReadRecord>,
    pub write_records: Vec<MemoryWriteRecord>,
    /// The local memory access records.
    pub local_mem_access: Vec<MemoryLocalEvent>,
}
