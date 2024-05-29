use std::borrow::{Borrow, BorrowMut};

use p3_air::{Air, AirBuilder, BaseAir};
use p3_field::AbstractField;
use p3_field::{Field, PrimeField32};
use p3_matrix::{dense::RowMajorMatrix, Matrix};
use serde::{Deserialize, Serialize};
use sp1_derive::AlignedBorrow;

use crate::{
    air::MachineAir,
    runtime::{ExecutionRecord, Program, Syscall, SyscallCode},
    stark::SP1AirBuilder,
};

const NUM_COLS: usize = core::mem::size_of::<SyscallMarkerCols<u8>>();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallMarkerEvent {
    pub shard: u32,
    pub clk: u32,
    pub is_in: bool,
    pub a: u32,
    pub b: u32,
}

#[derive(Debug, Clone, AlignedBorrow)]
#[repr(C)]
pub struct SyscallMarkerCols<T> {
    is_real: T,
    shard: T,
    clk: T,
    is_in: T,
    a: T,
    b: T,
}

pub struct SyscallMarkerChip {
    is_in: bool,
}

impl SyscallMarkerChip {
    pub fn new_in() -> Self {
        Self { is_in: true }
    }

    pub fn new_out() -> Self {
        Self { is_in: false }
    }
}

impl Syscall for SyscallMarkerChip {
    fn execute(&self, rt: &mut crate::runtime::SyscallContext, a: u32, b: u32) -> Option<u32> {
        let event = SyscallMarkerEvent {
            shard: rt.current_shard(),
            clk: rt.clk,
            is_in: self.is_in,
            a,
            b,
        };
        rt.record_mut().syscall_marker_events.push(event);
        None
    }
}

impl<F: PrimeField32> MachineAir<F> for SyscallMarkerChip {
    type Record = ExecutionRecord;

    type Program = Program;

    fn name(&self) -> String {
        "SyscallMarker".to_string()
    }

    fn generate_trace(
        &self,
        input: &Self::Record,
        _output: &mut Self::Record,
    ) -> RowMajorMatrix<F> {
        let mut rows = vec![];

        for event in input.syscall_marker_events.iter() {
            let mut row = [F::zero(); NUM_COLS];
            let cols: &mut SyscallMarkerCols<F> = row.as_mut_slice().borrow_mut();

            cols.is_real = F::one();
            cols.shard = F::from_canonical_u32(event.shard);
            cols.clk = F::from_canonical_u32(event.clk);
            cols.is_in = F::from_bool(event.is_in);
            cols.a = F::from_canonical_u32(event.a);
            cols.b = F::from_canonical_u32(event.b);

            rows.push(row);
        }

        RowMajorMatrix::new(rows.into_iter().flatten().collect::<Vec<_>>(), NUM_COLS)
    }

    fn included(&self, shard: &Self::Record) -> bool {
        !shard.syscall_marker_events.is_empty()
    }
}

impl<F: Field> BaseAir<F> for SyscallMarkerChip {
    fn width(&self) -> usize {
        NUM_COLS
    }
}

impl<AB> Air<AB> for SyscallMarkerChip
where
    AB: SP1AirBuilder,
{
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let row = main.row_slice(0);
        let row: &SyscallMarkerCols<AB::Var> = (*row).borrow();

        let syscall_id = builder.if_else(
            row.is_in,
            AB::F::from_canonical_u32(SyscallCode::MARKER_IN as u32),
            AB::F::from_canonical_u32(SyscallCode::MARKER_OUT as u32),
        );
        builder.receive_syscall(row.shard, row.clk, syscall_id, row.a, row.b, row.is_real);
    }
}
