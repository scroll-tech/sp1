mod general_field_op;

use std::borrow::{Borrow, BorrowMut};

use crate::{
    air::MachineAir,
    memory::{MemoryCols, MemoryReadCols, MemoryWriteCols},
    operations::field::{field_op::FieldOperation, params::Limbs},
    runtime::{
        ExecutionRecord, MemoryReadRecord, MemoryWriteRecord, Program, Syscall, SyscallCode,
    },
    stark::SP1AirBuilder,
    utils::{
        ec::{
            field::{FieldParameters, NumLimbs, NumWords},
            weierstrass::bn254::Bn254ScalarField,
        },
        limbs_from_prev_access,
    },
};
use num::BigUint;
use p3_air::{Air, AirBuilder, BaseAir};
use p3_field::{AbstractField, Field, PrimeField32};
use p3_matrix::dense::RowMajorMatrix;
use p3_matrix::Matrix;
use sp1_derive::AlignedBorrow;
use typenum::Unsigned;

use serde::{Deserialize, Serialize};

use self::general_field_op::GeneralFieldOpCols;

const NUM_WORDS_PER_FE: usize = 8;
const NUM_COLS: usize = core::mem::size_of::<Bn254ScalarArithAssignCols<u8>>();

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldArithEvent {
    pub shard: u32,
    pub clk: u32,
    pub op: FieldOperation,
    pub p: Vec<u32>,
    pub q: Vec<u32>,
    pub pq_ptr: u32,
    pub op_ptr: u32,
    pub p_memory_records: Vec<MemoryWriteRecord>,
    pub q_memory_records: Vec<MemoryReadRecord>,
    pub op_memory_record: MemoryReadRecord,
}

#[derive(Debug, Clone, AlignedBorrow)]
#[repr(C)]
pub struct Bn254ScalarArithAssignCols<T> {
    shard: T,
    clk: T,
    op: T,
    pq_ptr: T,
    op_ptr: T,
    p_access: [MemoryWriteCols<T>; NUM_WORDS_PER_FE],
    q_access: [MemoryReadCols<T>; NUM_WORDS_PER_FE],
    op_access: MemoryReadCols<T>,
    eval: GeneralFieldOpCols<T, Bn254ScalarField>,
}

pub struct Bn254ScalarArithChip;

impl Bn254ScalarArithChip {
    pub fn new() -> Self {
        Self
    }
}

impl Syscall for Bn254ScalarArithChip {
    fn execute(
        &self,
        rt: &mut crate::runtime::SyscallContext,
        arg1: u32,
        arg2: u32,
    ) -> Option<u32> {
        // layout of input to this syscall
        // | p (8 words) | q (8 words) | type (1 word) |
        let start_clk = rt.clk;

        let pq_ptr = arg1;
        if pq_ptr % 4 != 0 {
            panic!();
        }
        let op_ptr = arg2;
        if op_ptr % 4 != 0 {
            panic!();
        }

        let nw_per_fe = <Bn254ScalarField as NumWords>::WordsFieldElement::USIZE;
        debug_assert_eq!(nw_per_fe, NUM_WORDS_PER_FE);

        let p: Vec<u32> = rt.slice_unsafe(pq_ptr, nw_per_fe);

        let (q_memory_records, q) = rt.mr_slice(pq_ptr + 4 * (nw_per_fe as u32), nw_per_fe);
        let (op_memory_record, op) = rt.mr(op_ptr);

        // TODO: why?
        rt.clk += 1;

        let bn_p = BigUint::from_bytes_le(
            &p.iter()
                .cloned()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<u8>>(),
        );
        let bn_q = BigUint::from_bytes_le(
            &q.iter()
                .cloned()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<u8>>(),
        );

        let modulus = Bn254ScalarField::modulus();
        let (r, t) = match op {
            0x00 => ((&bn_p + &bn_q) % modulus, FieldOperation::Add),
            0x01 => ((&bn_p - &bn_q) % modulus, FieldOperation::Sub),
            0x10 => ((&bn_p * &bn_q) % modulus, FieldOperation::Mul),
            // TODO: how to handle q == 0?
            0x11 => ((&bn_p / &bn_q) % modulus, FieldOperation::Div),
            _ => unreachable!("type {} not supported", op),
        };

        let mut result_words = r.to_u32_digits();
        result_words.resize(nw_per_fe, 0);

        let p_memory_records = rt.mw_slice(pq_ptr, &result_words);

        let shard = rt.current_shard();
        rt.record_mut()
            .bn254_scalar_arith_events
            .push(FieldArithEvent {
                shard,
                clk: start_clk,
                op: t,
                p,
                q,
                op_ptr,
                pq_ptr,
                p_memory_records,
                q_memory_records,
                op_memory_record,
            });

        None
    }
}

impl<F: PrimeField32> MachineAir<F> for Bn254ScalarArithChip {
    type Record = ExecutionRecord;

    type Program = Program;

    fn name(&self) -> String {
        "Bn254ScalarArith".to_string()
    }

    fn generate_trace(&self, input: &Self::Record, output: &mut Self::Record) -> RowMajorMatrix<F> {
        let mut rows = vec![];
        let mut new_byte_lookup_events = vec![];

        for event in input.bn254_scalar_arith_events.iter() {
            let mut row = [F::zero(); NUM_COLS];
            let cols: &mut Bn254ScalarArithAssignCols<F> = row.as_mut_slice().borrow_mut();

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

            cols.shard = F::from_canonical_u32(event.shard);
            cols.clk = F::from_canonical_u32(event.clk);
            cols.pq_ptr = F::from_canonical_u32(event.pq_ptr);
            cols.op_ptr = F::from_canonical_u32(event.op_ptr);
            cols.op = F::from_canonical_u32(event.op as u32);

            cols.eval.populate(&p, &q, event.op);

            for i in 0..cols.p_access.len() {
                cols.p_access[i].populate(event.p_memory_records[i], &mut new_byte_lookup_events);
            }
            for i in 0..cols.q_access.len() {
                cols.q_access[i].populate(event.q_memory_records[i], &mut new_byte_lookup_events);
            }
            cols.op_access
                .populate(event.op_memory_record, &mut new_byte_lookup_events);

            rows.push(row);
        }
        output.add_byte_lookup_events(new_byte_lookup_events);

        // TODO: add padding rows

        RowMajorMatrix::new(rows.into_iter().flatten().collect::<Vec<_>>(), NUM_COLS)
    }

    fn included(&self, shard: &Self::Record) -> bool {
        !shard.bn254_scalar_arith_events.is_empty()
    }
}

impl<F: Field> BaseAir<F> for Bn254ScalarArithChip {
    fn width(&self) -> usize {
        NUM_COLS
    }
}

impl<AB> Air<AB> for Bn254ScalarArithChip
where
    AB: SP1AirBuilder,
    AB::Expr: Copy,
{
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let row = main.row_slice(0);
        let row: &Bn254ScalarArithAssignCols<AB::Var> = (*row).borrow();

        let p: Limbs<<AB as AirBuilder>::Var, <Bn254ScalarField as NumLimbs>::Limbs> =
            limbs_from_prev_access(&row.p_access);
        let q: Limbs<<AB as AirBuilder>::Var, <Bn254ScalarField as NumLimbs>::Limbs> =
            limbs_from_prev_access(&row.q_access);
        let op = row.op_access.prev_value().0.to_vec();

        row.eval.eval(builder, &p, &q, op[0]);

        for i in 0..Bn254ScalarField::NB_LIMBS {
            builder.assert_eq(row.eval.cols.result[i], row.p_access[i / 4].value()[i % 4]);
        }

        builder.eval_memory_access_slice(
            row.shard,
            row.clk.into(),
            row.pq_ptr + AB::F::from_canonical_u32(4),
            &row.q_access,
            AB::F::one(),
        );

        builder.eval_memory_access(row.shard, row.clk, row.op_ptr, &row.op_access, AB::F::one());

        builder.eval_memory_access_slice(
            row.shard,
            row.clk.into(),
            row.pq_ptr,
            &row.p_access,
            AB::F::one(),
        );

        let syscall_id = AB::F::from_canonical_u32(SyscallCode::BN254_SCALAR_ARITH.syscall_id());
        builder.receive_syscall(
            row.shard,
            row.clk,
            syscall_id,
            row.pq_ptr,
            row.op_ptr,
            AB::F::one(),
        );
    }
}

mod tests {
    use crate::{
        runtime::Program,
        utils::{run_test, setup_logger, tests::BN254_SCALAR_ARITH_ELF},
    };

    #[test]
    fn test_bn254_scalar_arith_simple() {
        setup_logger();
        let program = Program::from(BN254_SCALAR_ARITH_ELF);
        run_test(program).unwrap();
    }
}
