use std::marker::PhantomData;

use generic_array::ArrayLength;

use crate::{
    events::{MemCopyEvent, PrecompileEvent},
    syscalls::{Syscall, SyscallCode, SyscallContext},
};

pub struct MemCopySyscall<NumWords: ArrayLength, NumBytes: ArrayLength> {
    _marker: PhantomData<(NumWords, NumBytes)>,
}

impl<NumWords: ArrayLength, NumBytes: ArrayLength> MemCopySyscall<NumWords, NumBytes> {
    pub const fn new() -> Self {
        Self { _marker: PhantomData }
    }
}

impl<NumWords: ArrayLength + Send + Sync, NumBytes: ArrayLength + Send + Sync> Syscall
    for MemCopySyscall<NumWords, NumBytes>
{
    fn execute(
        &self,
        rt: &mut SyscallContext,
        syscall_code: SyscallCode,
        src: u32,
        dst: u32,
    ) -> Option<u32> {
        let (read, read_bytes) = rt.mr_slice(src, NumWords::USIZE);

        // dst == src is supported, even it is actually a no-op.
        rt.clk += 1;

        let write = rt.mw_slice(dst, &read_bytes);

        let event = MemCopyEvent {
            lookup_id: rt.syscall_lookup_id,
            shard: rt.current_shard(),
            clk: rt.clk,
            src_ptr: src,
            dst_ptr: dst,
            read_records: read,
            write_records: write,
            local_mem_access: rt.postprocess(),
        };
        rt.record_mut().add_precompile_event(
            syscall_code,
            match NumWords::USIZE {
                8 => PrecompileEvent::MemCopy32(event),
                16 => PrecompileEvent::MemCopy64(event),
                _ => panic!("invalid uszie {}", NumWords::USIZE),
            },
        );

        None
    }

    fn num_extra_cycles(&self) -> u32 {
        1
    }
}
