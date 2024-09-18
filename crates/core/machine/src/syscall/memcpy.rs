use generic_array::{ArrayLength, GenericArray};
use sp1_core_executor::events::ByteRecord;
use sp1_core_executor::events::MemCopyEvent;
use sp1_core_executor::syscalls::{Syscall, SyscallCode, SyscallContext};
use sp1_core_executor::{ExecutionRecord, Program};
use sp1_curves::params::Limbs;
use sp1_stark::air::{MachineAir, SP1AirBuilder};
use std::borrow::{Borrow, BorrowMut};
use std::marker::PhantomData;

use p3_air::{Air, AirBuilder, BaseAir};
use p3_field::AbstractField;
use p3_field::{Field, PrimeField32};
use p3_matrix::{dense::RowMajorMatrix, Matrix};
use sp1_derive::AlignedBorrow;

use crate::air::MemoryAirBuilder;
use crate::memory::{MemoryReadCols, MemoryWriteCols};
use crate::utils::{limbs_from_access, limbs_from_prev_access, pad_rows};

#[derive(Debug, Clone, AlignedBorrow)]
#[repr(C)]
pub struct MemCopyCols<T, NumWords: ArrayLength> {
    is_real: T,
    shard: T,
    channel: T,
    clk: T,
    nonce: T,
    src_ptr: T,
    dst_ptr: T,
    src_access: GenericArray<MemoryReadCols<T>, NumWords>,
    dst_access: GenericArray<MemoryWriteCols<T>, NumWords>,
}

pub struct MemCopyChip<NumWords: ArrayLength, NumBytes: ArrayLength> {
    _marker: PhantomData<(NumWords, NumBytes)>,
}

use typenum::{U16, U32, U64, U8};
pub type MemCopy32Chip = MemCopyChip<U8, U32>;
pub type MemCopy64Chip = MemCopyChip<U16, U64>;

impl<NumWords: ArrayLength, NumBytes: ArrayLength> MemCopyChip<NumWords, NumBytes> {
    const NUM_COLS: usize = core::mem::size_of::<MemCopyCols<u8, NumWords>>();

    pub fn new() -> Self {
        println!("MemCopyChip<{}> NUM_COLS = {}", NumWords::USIZE, Self::NUM_COLS);
        assert_eq!(NumWords::USIZE * 4, NumBytes::USIZE);
        Self { _marker: PhantomData }
    }

    pub fn syscall_id() -> u32 {
        match NumBytes::USIZE {
            32 => SyscallCode::MEMCPY_32.syscall_id(),
            64 => SyscallCode::MEMCPY_64.syscall_id(),
            _ => unreachable!(),
        }
    }
}

impl<F: PrimeField32, NumWords: ArrayLength + Send + Sync, NumBytes: ArrayLength + Send + Sync>
    MachineAir<F> for MemCopyChip<NumWords, NumBytes>
{
    type Record = ExecutionRecord;

    type Program = Program;

    fn name(&self) -> String {
        format!("MemCopy{}Chip", NumWords::USIZE)
    }

    fn generate_trace(&self, input: &Self::Record, output: &mut Self::Record) -> RowMajorMatrix<F> {
        let mut rows = vec![];
        let mut new_byte_lookup_events = vec![];
        let events = match NumWords::USIZE {
            8 => &input.memcpy32_events,
            16 => &input.memcpy64_events,
            _ => unreachable!(),
        };

        for event in events {
            let mut row = Vec::with_capacity(Self::NUM_COLS);
            row.resize(Self::NUM_COLS, F::zero());
            let cols: &mut MemCopyCols<F, NumWords> = row.as_mut_slice().borrow_mut();

            cols.is_real = F::one();
            cols.shard = F::from_canonical_u32(event.shard);
            cols.channel = F::from_canonical_u8(event.channel);
            cols.clk = F::from_canonical_u32(event.clk);
            cols.src_ptr = F::from_canonical_u32(event.src_ptr);
            cols.dst_ptr = F::from_canonical_u32(event.dst_ptr);

            /*
                cols.nonce = F::from_canonical_u32(
                    output
                        .nonce_lookup
                        .get(&event.lookup_id)
                        .copied()
                        .expect("should not be none"),
                );
            */

            for i in 0..NumWords::USIZE {
                cols.src_access[i].populate(
                    event.channel,
                    event.read_records[i],
                    &mut new_byte_lookup_events,
                );
            }
            for i in 0..NumWords::USIZE {
                cols.dst_access[i].populate(
                    event.channel,
                    event.write_records[i],
                    &mut new_byte_lookup_events,
                );
            }

            rows.push(row);
        }
        output.add_byte_lookup_events(new_byte_lookup_events);

        pad_rows(&mut rows, || vec![F::zero(); Self::NUM_COLS]);

        let mut trace =
            RowMajorMatrix::new(rows.into_iter().flatten().collect::<Vec<_>>(), Self::NUM_COLS);
        // Write the nonces to the trace.
        for i in 0..trace.height() {
            let cols: &mut MemCopyCols<F, NumWords> =
                trace.values[i * Self::NUM_COLS..(i + 1) * Self::NUM_COLS].borrow_mut();
            //cols.nonce = F::from_canonical_usize(i);
        }
        trace
    }

    fn included(&self, shard: &Self::Record) -> bool {
        !(match NumWords::USIZE {
            8 => &shard.memcpy32_events,
            16 => &shard.memcpy64_events,
            _ => unreachable!(),
        })
        .is_empty()
    }
}

impl<F: Field, NumWords: ArrayLength + Sync, NumBytes: ArrayLength + Sync> BaseAir<F>
    for MemCopyChip<NumWords, NumBytes>
{
    fn width(&self) -> usize {
        Self::NUM_COLS
    }
}

impl<AB: SP1AirBuilder, NumWords: ArrayLength + Sync, NumBytes: ArrayLength + Sync> Air<AB>
    for MemCopyChip<NumWords, NumBytes>
{
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let row = main.row_slice(0);
        let row: &MemCopyCols<AB::Var, NumWords> = (*row).borrow();

        let src: Limbs<<AB as AirBuilder>::Var, NumBytes> = limbs_from_prev_access(&row.src_access);
        let dst: Limbs<<AB as AirBuilder>::Var, NumBytes> = limbs_from_access(&row.dst_access);

        // TODO assert eq

        builder.eval_memory_access_slice(
            row.shard,
            row.channel,
            row.clk.into(),
            row.src_ptr,
            &row.src_access,
            row.is_real,
        );
        builder.eval_memory_access_slice(
            row.shard,
            row.channel,
            row.clk.into(),
            row.dst_ptr,
            &row.dst_access,
            row.is_real,
        );

        builder.receive_syscall(
            row.shard,
            row.channel,
            row.clk,
            row.nonce,
            AB::F::from_canonical_u32(Self::syscall_id()),
            row.src_ptr,
            row.dst_ptr,
            row.is_real,
        );
    }
}
