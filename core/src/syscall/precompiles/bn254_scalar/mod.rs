use crate::{
    memory::{MemoryReadCols, MemoryWriteCols},
    operations::field::field_op::{FieldOpCols, FieldOperation},
    runtime::{MemoryReadRecord, MemoryWriteRecord, Syscall},
    utils::ec::{
        field::{FieldParameters, NumWords},
        weierstrass::bn254::Bn254ScalarField,
    },
};
use num::BigUint;
use typenum::Unsigned;

use serde::{Deserialize, Serialize};

const NUM_WORDS_PER_FE: usize = 8;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldArithEvent {
    pub shard: u32,
    pub clk: u32,
    pub op: FieldOperation,
    pub p: Vec<u32>,
    pub q: Vec<u32>,
    pub p_memory_records: Vec<MemoryWriteRecord>,
    pub q_memory_records: Vec<MemoryReadRecord>,
    pub op_memory_record: MemoryReadRecord,
}

pub struct Bn254ScalarArithAssignCols<T> {
    shard: T,
    clk: T,
    op: T,
    pq_ptr: T,
    op_ptr: T,
    p_memory_records: [MemoryWriteCols<T>; NUM_WORDS_PER_FE],
    q_memory_records: [MemoryReadCols<T>; NUM_WORDS_PER_FE],
    op_memory_record: MemoryReadCols<T>,
    eval: FieldOpCols<T, Bn254ScalarField>,
}

pub struct Bn254ScalarArithChip {}

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

        let (q_memory_records, q) = rt.mr_slice(pq_ptr + 4 * u32::from(nw_per_fe), nw_per_fe);
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
                p_memory_records,
                q_memory_records,
                op_memory_record,
            });

        None
    }
}
