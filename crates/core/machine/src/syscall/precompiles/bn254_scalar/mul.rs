use std::borrow::{Borrow, BorrowMut};

use num::BigUint;
use num::Zero;
use p3_air::{Air, AirBuilder, BaseAir};
use p3_field::AbstractField;
use p3_field::{Field, PrimeField32};
use p3_matrix::{dense::RowMajorMatrix, Matrix};
use sp1_core_executor::events::Bn254FieldOperation;
use sp1_core_executor::events::ByteRecord;
use sp1_core_executor::events::PrecompileEvent;
use sp1_core_executor::events::NUM_WORDS_PER_FE;
use sp1_core_executor::syscalls::SyscallCode;
use sp1_core_executor::ExecutionRecord;
use sp1_core_executor::Program;
use sp1_curves::params::FieldParameters;
use sp1_curves::params::Limbs;
use sp1_curves::params::NumLimbs;
use sp1_curves::weierstrass::bn254::Bn254ScalarField;
use sp1_derive::AlignedBorrow;
use sp1_stark::air::InteractionScope;
use sp1_stark::air::MachineAir;
use sp1_stark::air::SP1AirBuilder;

use crate::air::MemoryAirBuilder;
use crate::utils::limbs_from_prev_access;
use crate::utils::pad_rows_fixed;
use crate::{
    memory::{MemoryCols, MemoryReadCols, MemoryWriteCols},
    operations::field::field_op::FieldOpCols,
};

const NUM_COLS: usize = core::mem::size_of::<Bn254ScalarMulCols<u8>>();
const OP: Bn254FieldOperation = Bn254FieldOperation::Mul;

#[derive(Debug, Clone, AlignedBorrow)]
#[repr(C)]
pub struct Bn254ScalarMulCols<T> {
    is_real: T,
    shard: T,
    channel: T,
    nonce: T,
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

impl<F: PrimeField32> MachineAir<F> for Bn254ScalarMulChip {
    type Record = ExecutionRecord;

    type Program = Program;

    fn name(&self) -> String {
        "Bn254ScalarMul".to_string()
    }

    fn generate_trace(&self, input: &Self::Record, output: &mut Self::Record) -> RowMajorMatrix<F> {
        let events = input.get_precompile_events(SyscallCode::BN254_SCALAR_MUL);

        let mut rows = vec![];
        let mut new_byte_lookup_events = vec![];

        for event in events {
            let event = if let PrecompileEvent::Bn254ScalarMul(event) = event {
                event
            } else {
                unreachable!();
            };
            let mut row = [F::zero(); NUM_COLS];
            let cols: &mut Bn254ScalarMulCols<F> = row.as_mut_slice().borrow_mut();

            let p = event.arg1.prev_value_as_biguint();
            let q = event.arg2.value_as_biguint();

            cols.is_real = F::one();
            cols.shard = F::from_canonical_u32(event.shard);
            cols.clk = F::from_canonical_u32(event.clk);
            cols.p_ptr = F::from_canonical_u32(event.arg1.ptr);
            cols.q_ptr = F::from_canonical_u32(event.arg2.ptr);

            /*
                cols.nonce = F::from_canonical_u32(
                    output
                        .nonce_lookup
                        .get(&event.lookup_id)
                        .copied()
                        .expect("should not be none"),
                );
            */

            cols.eval.populate(
                &mut new_byte_lookup_events,
                event.shard,
                &p,
                &q,
                OP.to_field_operation(),
            );

            for i in 0..cols.p_access.len() {
                cols.p_access[i]
                    .populate(event.arg1.memory_records[i], &mut new_byte_lookup_events);
            }
            for i in 0..cols.q_access.len() {
                cols.q_access[i]
                    .populate(event.arg2.memory_records[i], &mut new_byte_lookup_events);
            }

            rows.push(row);
        }
        output.add_byte_lookup_events(new_byte_lookup_events);

        pad_rows_fixed(
            &mut rows,
            || {
                let mut row = [F::zero(); NUM_COLS];
                let cols: &mut Bn254ScalarMulCols<F> = row.as_mut_slice().borrow_mut();

                let zero = BigUint::zero();
                cols.eval.populate(&mut vec![], 0, &zero, &zero, OP.to_field_operation());

                row
            },
            input.fixed_log2_rows::<F, _>(self),
        );

        let mut trace =
            RowMajorMatrix::new(rows.into_iter().flatten().collect::<Vec<_>>(), NUM_COLS);
        // Write the nonces to the trace.
        for i in 0..trace.height() {
            let _cols: &mut Bn254ScalarMulCols<F> =
                trace.values[i * NUM_COLS..(i + 1) * NUM_COLS].borrow_mut();
            //cols.nonce = F::from_canonical_usize(i);
        }

        trace
    }

    fn included(&self, shard: &Self::Record) -> bool {
        !shard.get_precompile_events(SyscallCode::BN254_SCALAR_MUL).is_empty()
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

        row.eval.eval(builder, &p, &q, OP.to_field_operation(), row.is_real);

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
            row.nonce,
            syscall_id,
            row.p_ptr,
            row.q_ptr,
            row.is_real,
            InteractionScope::Global,
        );
    }
}
