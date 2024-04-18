use crate::{
    operations::field::field_op::{FieldOpCols, FieldOperation},
    runtime::Syscall,
    utils::ec::{
        field::{FieldParameters, NumWords},
        weierstrass::bn254::Bn254ScalarField,
    },
};
use num::BigUint;
use typenum::Unsigned;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldArithEvent {
    pub shard: u32,
    pub clk: u32,
    pub op: FieldOperation,
    pub p: Vec<u32>,
    pub q: Vec<u32>,
}

pub struct Bn254ScalarArithAssignCols<T> {
    shard: T,
    clk: T,
    p_ptr: T,
    q_ptr: T,
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
        // layout of this syscall
        // | p (8 words) | q (8 words) | type (1 word) |
        let start_clk = rt.clk;

        let ops_ptr = arg1;
        if ops_ptr % 4 != 0 {
            panic!();
        }
        let type_ptr = arg2;
        if type_ptr % 4 != 0 {
            panic!();
        }

        let nw_per_fe = <Bn254ScalarField as NumWords>::WordsFieldElement::USIZE;

        let (ops_memory_records, ops) = rt.mr_slice(ops_ptr, nw_per_fe * 2);

        let (t_memory_record, t) = rt.mr(type_ptr);

        // TODO: why?
        rt.clk += 1;

        let p = BigUint::from_bytes_le(
            &ops.iter()
                .take(nw_per_fe)
                .cloned()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<u8>>(),
        );
        let q = BigUint::from_bytes_le(
            &ops.iter()
                .skip(nw_per_fe)
                .cloned()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<u8>>(),
        );

        let modulus = Bn254ScalarField::modulus();
        let (r, t) = match t {
            0x00 => ((&p + &q) % modulus, FieldOperation::Add),
            0x01 => ((&p - &q) % modulus, FieldOperation::Sub),
            0x10 => ((&p * &q) % modulus, FieldOperation::Mul),
            0x11 => ((&p / &q) % modulus, FieldOperation::Div),
            _ => unreachable!("type {} not supported", t),
        };

        let mut result_words = r.to_u32_digits();
        result_words.resize(nw_per_fe, 0);

        rt.mw_slice(ops_ptr, &result_words);

        let shard = rt.current_shard();
        rt.record_mut()
            .bn254_scalar_arith_events
            .push(FieldArithEvent {
                shard,
                clk: start_clk,
                op: t,
                p: ops.iter().cloned().take(nw_per_fe).collect(),
                q: ops.iter().skip(nw_per_fe).cloned().collect(),
            });

        None
    }
}
