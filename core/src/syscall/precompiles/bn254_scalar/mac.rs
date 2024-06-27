use std::borrow::{Borrow, BorrowMut};

use num::BigUint;
use num::Zero;
use p3_air::{Air, AirBuilder, BaseAir};
use p3_field::AbstractField;
use p3_field::{Field, PrimeField32};
use p3_matrix::{dense::RowMajorMatrix, Matrix};
use sp1_derive::AlignedBorrow;
use typenum::U8;

use crate::{
    air::MachineAir,
    bytes::event::ByteRecord,
    memory::{MemoryCols, MemoryReadCols, MemoryWriteCols},
    operations::field::field_op::{FieldOpCols, FieldOperation},
    operations::field::params::{FieldParameters, NumLimbs},
    runtime::{ExecutionRecord, Program, Syscall, SyscallCode},
    stark::SP1AirBuilder,
    syscall::precompiles::bn254_scalar::Limbs,
    utils::{ec::weierstrass::bn254::Bn254ScalarField, limbs_from_prev_access, pad_rows},
};

use super::{create_bn254_scalar_arith_event, Bn254FieldOperation, NUM_WORDS_PER_FE};

const NUM_COLS: usize = core::mem::size_of::<Bn254ScalarMacCols<u8>>();
const OP: Bn254FieldOperation = Bn254FieldOperation::Mac;

#[derive(Debug, Clone, AlignedBorrow)]
#[repr(C)]
pub struct Bn254ScalarMacCols<T> {
    is_real: T,
    shard: T,
    channel: T,
    clk: T,
    arg1_ptr: T,
    arg2_ptr: T,
    arg1_access: [MemoryWriteCols<T>; NUM_WORDS_PER_FE],
    arg2_access: [MemoryReadCols<T>; 2],
    a_access: [MemoryReadCols<T>; NUM_WORDS_PER_FE],
    b_access: [MemoryReadCols<T>; NUM_WORDS_PER_FE],
    mul_eval: FieldOpCols<T, Bn254ScalarField>,
    add_eval: FieldOpCols<T, Bn254ScalarField>,
}

pub struct Bn254ScalarMacChip;

impl Bn254ScalarMacChip {
    pub fn new() -> Self {
        Self
    }
}

impl Syscall for Bn254ScalarMacChip {
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

impl<F: PrimeField32> MachineAir<F> for Bn254ScalarMacChip {
    type Record = ExecutionRecord;

    type Program = Program;

    fn name(&self) -> String {
        "Bn254ScalarMac".to_string()
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
            let cols: &mut Bn254ScalarMacCols<F> = row.as_mut_slice().borrow_mut();

            let arg1 = event.arg1.prev_value_as_biguint();
            let a = event.a.as_ref().unwrap().value_as_biguint();
            let b = event.b.as_ref().unwrap().value_as_biguint();

            cols.is_real = F::one();
            cols.shard = F::from_canonical_u32(event.shard);
            cols.channel = F::from_canonical_u32(event.channel);
            cols.clk = F::from_canonical_u32(event.clk);
            cols.arg1_ptr = F::from_canonical_u32(event.arg1.ptr);
            cols.arg2_ptr = F::from_canonical_u32(event.arg2.ptr);

            let mul = cols.mul_eval.populate(
                &mut new_byte_lookup_events,
                event.shard,
                event.channel,
                &a,
                &b,
                FieldOperation::Mul,
            );
            cols.add_eval.populate(
                &mut new_byte_lookup_events,
                event.shard,
                event.channel,
                &arg1,
                &mul,
                FieldOperation::Add,
            );

            for i in 0..cols.arg1_access.len() {
                cols.arg1_access[i].populate(
                    event.channel,
                    event.arg1.memory_records[i],
                    &mut new_byte_lookup_events,
                );
            }
            for i in 0..cols.arg2_access.len() {
                cols.arg2_access[i].populate(
                    event.channel,
                    event.arg2.memory_records[i],
                    &mut new_byte_lookup_events,
                );
            }
            for i in 0..cols.a_access.len() {
                cols.a_access[i].populate(
                    event.channel,
                    event.a.as_ref().unwrap().memory_records[i],
                    &mut new_byte_lookup_events,
                );
            }
            for i in 0..cols.b_access.len() {
                cols.b_access[i].populate(
                    event.channel,
                    event.b.as_ref().unwrap().memory_records[i],
                    &mut new_byte_lookup_events,
                );
            }

            rows.push(row);
        }
        output.add_byte_lookup_events(new_byte_lookup_events);

        pad_rows(&mut rows, || {
            let mut row = [F::zero(); NUM_COLS];
            let cols: &mut Bn254ScalarMacCols<F> = row.as_mut_slice().borrow_mut();

            let zero = BigUint::zero();
            cols.mul_eval
                .populate(&mut vec![], 0, 0, &zero, &zero, FieldOperation::Mul);
            cols.add_eval
                .populate(&mut vec![], 0, 0, &zero, &zero, FieldOperation::Add);

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

impl<F: Field> BaseAir<F> for Bn254ScalarMacChip {
    fn width(&self) -> usize {
        NUM_COLS
    }
}

impl<AB> Air<AB> for Bn254ScalarMacChip
where
    AB: SP1AirBuilder,
{
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let row = main.row_slice(0);
        let row: &Bn254ScalarMacCols<AB::Var> = (*row).borrow();

        builder.assert_bool(row.is_real);

        let arg1: Limbs<<AB as AirBuilder>::Var, <Bn254ScalarField as NumLimbs>::Limbs> =
            limbs_from_prev_access(&row.arg1_access);
        let arg2: Limbs<<AB as AirBuilder>::Var, U8> = limbs_from_prev_access(&row.arg2_access);
        let a: Limbs<<AB as AirBuilder>::Var, <Bn254ScalarField as NumLimbs>::Limbs> =
            limbs_from_prev_access(&row.a_access);
        let b: Limbs<<AB as AirBuilder>::Var, <Bn254ScalarField as NumLimbs>::Limbs> =
            limbs_from_prev_access(&row.b_access);

        row.mul_eval.eval(
            builder,
            &a,
            &b,
            FieldOperation::Mul,
            row.shard,
            row.channel,
            row.is_real,
        );
        row.add_eval.eval(
            builder,
            &arg1,
            &row.mul_eval.result,
            FieldOperation::Add,
            row.shard,
            row.channel,
            row.is_real,
        );

        for i in 0..Bn254ScalarField::NB_LIMBS {
            builder.when(row.is_real).assert_eq(
                row.add_eval.result[i],
                row.arg1_access[i / 4].value()[i % 4],
            );
        }

        builder.eval_memory_access_slice(
            row.shard,
            row.channel,
            row.clk.into(),
            row.arg1_ptr,
            &row.arg1_access,
            row.is_real,
        );

        builder.eval_memory_access_slice(
            row.shard,
            row.channel,
            row.clk.into(),
            row.arg2_ptr,
            &row.arg2_access,
            row.is_real,
        );

        let a_ptr = arg2.0[0..4]
            .iter()
            .rev()
            .cloned()
            .map(|v| v.into())
            .fold(AB::Expr::zero(), |acc, b| {
                acc * AB::Expr::from_canonical_u16(0x100) + b
            });

        let b_ptr = arg2.0[4..8]
            .iter()
            .rev()
            .cloned()
            .map(|v| v.into())
            .fold(AB::Expr::zero(), |acc, b| {
                acc * AB::Expr::from_canonical_u16(0x100) + b
            });

        builder.eval_memory_access_slice(
            row.shard,
            row.channel,
            row.clk.into(),
            a_ptr,
            &row.a_access,
            row.is_real,
        );

        builder.eval_memory_access_slice(
            row.shard,
            row.channel,
            row.clk.into(),
            b_ptr,
            &row.b_access,
            row.is_real,
        );

        let syscall_id = AB::F::from_canonical_u32(SyscallCode::BN254_SCALAR_MAC.syscall_id());
        builder.receive_syscall(
            row.shard,
            row.channel,
            row.clk,
            AB::F::from_canonical_u32(0u32),
            syscall_id,
            row.arg1_ptr,
            row.arg2_ptr,
            row.is_real,
        );
    }
}
