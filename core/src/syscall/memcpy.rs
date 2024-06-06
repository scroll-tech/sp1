use std::borrow::{Borrow, BorrowMut};

use p3_air::{Air, BaseAir};
use p3_field::AbstractField;
use p3_field::{Field, PrimeField32};
use p3_matrix::{dense::RowMajorMatrix, Matrix};
use serde::{Deserialize, Serialize};
use sp1_derive::AlignedBorrow;

use crate::{
    air::MachineAir,
    memory::{MemoryReadCols, MemoryWriteCols},
    runtime::{
        ExecutionRecord, MemoryReadRecord, MemoryWriteRecord, Program, Syscall, SyscallCode,
    },
    stark::SP1AirBuilder,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemCopyEvent {
    pub shard: u32,
    pub clk: u32,
    pub src_ptr: u32,
    pub dst_ptr: u32,
    pub src_access: Vec<MemoryReadRecord>,
    pub dst_access: Vec<MemoryWriteRecord>,
}

#[derive(Debug, Clone, AlignedBorrow)]
#[repr(C)]
pub struct MemCopyCols<T, const NUM_WORDS: usize> {
    is_real: T,
    shard: T,
    clk: T,
    src_ptr: T,
    dst_ptr: T,
    src_access: [MemoryReadCols<T>; NUM_WORDS],
    dst_access: [MemoryWriteCols<T>; NUM_WORDS],
}

pub struct MemCopyChip<const NUM_WORDS: usize>;

impl<const NUM_WORDS: usize> MemCopyChip<NUM_WORDS> {
    const NUM_COLS: usize = core::mem::size_of::<MemCopyCols<u8, NUM_WORDS>>();

    pub fn new() -> Self {
        println!("MemCopyChip<{NUM_WORDS}> NUM_COLS = {}", Self::NUM_COLS);
        Self
    }

    pub fn syscall_id() -> u32 {
        match NUM_WORDS {
            8 => SyscallCode::MEMCPY_32.syscall_id(),
            16 => SyscallCode::MEMCPY_64.syscall_id(),
            _ => unreachable!(),
        }
    }
}

impl<const NUM_WORDS: usize> Syscall for MemCopyChip<NUM_WORDS> {
    fn execute(&self, ctx: &mut crate::runtime::SyscallContext, src: u32, dst: u32) -> Option<u32> {
        let (read, read_bytes) = ctx.mr_slice(src, NUM_WORDS);
        let write = ctx.mw_slice(dst, &read_bytes);

        let event = MemCopyEvent {
            shard: ctx.current_shard(),
            clk: ctx.clk,
            src_ptr: src,
            dst_ptr: dst,
            src_access: read,
            dst_access: write,
        };
        ctx.record_mut()
            .memcpy_events
            .entry(NUM_WORDS)
            .or_default()
            .push(event);

        None
    }
}

impl<F: PrimeField32, const NUM_WORDS: usize> MachineAir<F> for MemCopyChip<NUM_WORDS>
where
    [(); Self::NUM_COLS]:,
{
    type Record = ExecutionRecord;

    type Program = Program;

    fn name(&self) -> String {
        format!("MemCopy{}Chip", NUM_WORDS)
    }

    fn generate_trace(&self, input: &Self::Record, output: &mut Self::Record) -> RowMajorMatrix<F> {
        let mut rows = vec![];
        let mut new_byte_lookup_events = vec![];

        for event in input.memcpy_events.get(&NUM_WORDS).unwrap_or(&vec![]) {
            let mut row = [F::zero(); Self::NUM_COLS];
            let cols: &mut MemCopyCols<F, NUM_WORDS> = row.as_mut_slice().borrow_mut();

            cols.is_real = F::one();
            cols.shard = F::from_canonical_u32(event.shard);
            cols.clk = F::from_canonical_u32(event.clk);
            cols.src_ptr = F::from_canonical_u32(event.src_ptr);
            cols.dst_ptr = F::from_canonical_u32(event.dst_ptr);

            for i in 0..NUM_WORDS {
                cols.src_access[i].populate(event.src_access[i], &mut new_byte_lookup_events);
            }
            for i in 0..NUM_WORDS {
                cols.dst_access[i].populate(event.dst_access[i], &mut new_byte_lookup_events);
            }

            rows.push(row);
        }
        output.add_byte_lookup_events(new_byte_lookup_events);

        RowMajorMatrix::new(
            rows.into_iter().flatten().collect::<Vec<_>>(),
            Self::NUM_COLS,
        )
    }

    fn included(&self, shard: &Self::Record) -> bool {
        shard
            .memcpy_events
            .get(&NUM_WORDS)
            .map(|events| !events.is_empty())
            .unwrap_or(false)
    }
}

impl<F: Field, const NUM_WORDS: usize> BaseAir<F> for MemCopyChip<NUM_WORDS> {
    fn width(&self) -> usize {
        Self::NUM_COLS
    }
}

impl<AB: SP1AirBuilder, const NUM_WORDS: usize> Air<AB> for MemCopyChip<NUM_WORDS> {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let row = main.row_slice(0);
        let row: &MemCopyCols<AB::Var, NUM_WORDS> = (*row).borrow();

        // TODO assert eq

        builder.receive_syscall(
            row.shard,
            row.clk,
            AB::F::from_canonical_u32(Self::syscall_id()),
            row.src_ptr,
            row.dst_ptr,
            row.is_real,
        );
    }
}
