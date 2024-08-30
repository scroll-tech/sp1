use super::{LookupId, MemoryReadRecord, MemoryWriteRecord};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemCopyEvent {
    pub lookup_id: LookupId,
    pub shard: u32,
    pub channel: u8,
    pub clk: u32,
    pub src_ptr: u32,
    pub dst_ptr: u32,
    pub read_records: Vec<MemoryReadRecord>,
    pub write_records: Vec<MemoryWriteRecord>,
}
