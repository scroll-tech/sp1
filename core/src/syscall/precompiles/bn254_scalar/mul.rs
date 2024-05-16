use std::borrow::{Borrow, BorrowMut};

use num::BigUint;
use num::Zero;
use p3_air::{Air, AirBuilder, BaseAir};
use p3_field::AbstractField;
use p3_field::{Field, PrimeField32};
use p3_matrix::{dense::RowMajorMatrix, Matrix};
use sp1_derive::AlignedBorrow;

use crate::{
    air::MachineAir,
    memory::{MemoryCols, MemoryReadCols, MemoryWriteCols},
    operations::field::field_op::{FieldOpCols, FieldOperation},
    runtime::{ExecutionRecord, Program, Syscall, SyscallCode},
    stark::SP1AirBuilder,
    syscall::precompiles::bn254_scalar::Limbs,
    utils::{
        ec::{
            field::{FieldParameters, NumLimbs},
            weierstrass::bn254::Bn254ScalarField,
        },
        limbs_from_prev_access, pad_rows,
    },
};

use super::{create_bn254_scalar_arith_event, NUM_WORDS_PER_FE};

const NUM_COLS: usize = core::mem::size_of::<Bn254ScalarMulCols<u8>>();
const OP: FieldOperation = FieldOperation::Mul;

#[derive(Debug, Clone, AlignedBorrow)]
#[repr(C)]
pub struct Bn254ScalarMulCols<T> {
    is_real: T,
    shard: T,
    clk: T,
    p_ptr: T,
    q_ptr: T,
    p_access: [MemoryWriteCols<T>; NUM_WORDS_PER_FE],
    q_access: [MemoryReadCols<T>; NUM_WORDS_PER_FE],
    eval: FieldOpCols<T, Bn254ScalarField>,
}

pub struct Bn254ScalarMulChip;

impl Bn254ScalarMulChip {
    pub fn new() -> Self {
        Self
    }
}

impl Syscall for Bn254ScalarMulChip {
    fn execute(
        &self,
        rt: &mut crate::runtime::SyscallContext,
        arg1: u32,
        arg2: u32,
    ) -> Option<u32> {
        let event = create_bn254_scalar_arith_event(rt, arg1, arg2, OP);
        rt.record_mut().bn254_scalar_arith_events.push(event);

        None
    }

    fn num_extra_cycles(&self) -> u32 {
        1
    }
}

impl<F: PrimeField32> MachineAir<F> for Bn254ScalarMulChip {
    type Record = ExecutionRecord;

    type Program = Program;

    fn name(&self) -> String {
        "Bn254ScalarMul".to_string()
    }

    fn generate_trace(&self, input: &Self::Record, output: &mut Self::Record) -> RowMajorMatrix<F> {
        let mut rows = vec![];
        let mut new_byte_lookup_events = vec![];

        for event in input
            .bn254_scalar_arith_events
            .iter()
            .filter(|e| e.op == OP)
        {
            let mut row = [F::zero(); NUM_COLS];
            let cols: &mut Bn254ScalarMulCols<F> = row.as_mut_slice().borrow_mut();

            let p = BigUint::from_bytes_le(
                event
                    .p
                    .iter()
                    .flat_map(|p| p.to_le_bytes())
                    .collect::<Vec<_>>()
                    .as_slice(),
            );
            let q = BigUint::from_bytes_le(
                event
                    .q
                    .iter()
                    .flat_map(|q| q.to_le_bytes())
                    .collect::<Vec<_>>()
                    .as_slice(),
            );

            cols.is_real = F::one();
            cols.shard = F::from_canonical_u32(event.shard);
            cols.clk = F::from_canonical_u32(event.clk);
            cols.p_ptr = F::from_canonical_u32(event.p_ptr);
            cols.q_ptr = F::from_canonical_u32(event.q_ptr);

            cols.eval.populate(&p, &q, OP);

            for i in 0..cols.p_access.len() {
                cols.p_access[i].populate(event.p_memory_records[i], &mut new_byte_lookup_events);
            }
            for i in 0..cols.q_access.len() {
                cols.q_access[i].populate(event.q_memory_records[i], &mut new_byte_lookup_events);
            }

            rows.push(row);
        }
        output.add_byte_lookup_events(new_byte_lookup_events);

        pad_rows(&mut rows, || {
            let mut row = [F::zero(); NUM_COLS];
            let cols: &mut Bn254ScalarMulCols<F> = row.as_mut_slice().borrow_mut();

            let zero = BigUint::zero();
            cols.eval.populate(&zero, &zero, OP);

            row
        });

        RowMajorMatrix::new(rows.into_iter().flatten().collect::<Vec<_>>(), NUM_COLS)
    }

    fn included(&self, shard: &Self::Record) -> bool {
        shard
            .bn254_scalar_arith_events
            .iter()
            .filter(|e| e.op == OP)
            .count()
            != 0
    }
}

impl<F: Field> BaseAir<F> for Bn254ScalarMulChip {
    fn width(&self) -> usize {
        NUM_COLS
    }
}

impl<AB> Air<AB> for Bn254ScalarMulChip
where
    AB: SP1AirBuilder,
    // AB::Expr: Copy,
{
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let row = main.row_slice(0);
        let row: &Bn254ScalarMulCols<AB::Var> = (*row).borrow();

        builder.assert_bool(row.is_real);

        let p: Limbs<<AB as AirBuilder>::Var, <Bn254ScalarField as NumLimbs>::Limbs> =
            limbs_from_prev_access(&row.p_access);
        let q: Limbs<<AB as AirBuilder>::Var, <Bn254ScalarField as NumLimbs>::Limbs> =
            limbs_from_prev_access(&row.q_access);

        row.eval.eval(builder, &p, &q, OP);

        for i in 0..Bn254ScalarField::NB_LIMBS {
            builder
                .when(row.is_real)
                .assert_eq(row.eval.result[i], row.p_access[i / 4].value()[i % 4]);
        }

        builder.eval_memory_access_slice(
            row.shard,
            row.clk.into(),
            row.q_ptr,
            &row.q_access,
            row.is_real,
        );

        builder.eval_memory_access_slice(
            row.shard,
            row.clk.into(),
            row.p_ptr,
            &row.p_access,
            row.is_real,
        );

        let syscall_id = AB::F::from_canonical_u32(SyscallCode::BN254_SCALAR_MUL.syscall_id());
        builder.receive_syscall(
            row.shard,
            row.clk,
            syscall_id,
            row.p_ptr,
            row.q_ptr,
            row.is_real,
        );
    }
}
