mod add;
mod mul;
// mod general_field_op;

pub use add::Bn254ScalarAddChip;
pub use mul::Bn254ScalarMulChip;

use crate::{
    operations::field::{field_op::FieldOperation, params::Limbs},
    runtime::{MemoryReadRecord, MemoryWriteRecord, SyscallContext},
    utils::ec::{
        field::{FieldParameters, NumWords},
        weierstrass::bn254::Bn254ScalarField,
    },
};
use num::BigUint;
use typenum::Unsigned;

use serde::{Deserialize, Serialize};

pub(crate) const NUM_WORDS_PER_FE: usize = 8;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldArithEvent {
    pub shard: u32,
    pub clk: u32,
    pub op: FieldOperation,
    pub p: Vec<u32>,
    pub q: Vec<u32>,
    pub p_ptr: u32,
    pub q_ptr: u32,
    pub p_memory_records: Vec<MemoryWriteRecord>,
    pub q_memory_records: Vec<MemoryReadRecord>,
}

pub fn create_bn254_scalar_arith_event(
    rt: &mut SyscallContext,
    arg1: u32,
    arg2: u32,
    op: FieldOperation,
) -> FieldArithEvent {
    let start_clk = rt.clk;
    let p_ptr = arg1;
    let q_ptr = arg2;

    assert_eq!(p_ptr % 4, 0, "p_ptr({:x}) is not aligned", p_ptr);
    assert_eq!(q_ptr % 4, 0, "q_ptr({:x}) is not aligned", q_ptr);

    let nw_per_fe = <Bn254ScalarField as NumWords>::WordsFieldElement::USIZE;
    debug_assert_eq!(nw_per_fe, NUM_WORDS_PER_FE);

    let p: Vec<u32> = rt.slice_unsafe(p_ptr, nw_per_fe);
    let (q_memory_records, q) = rt.mr_slice(q_ptr, nw_per_fe);

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

    let r = match op {
        FieldOperation::Add => (&bn_p + &bn_q) % modulus,
        FieldOperation::Mul => (&bn_p * &bn_q) % modulus,
        _ => unimplemented!("not supported"),
    };
    log::trace!(
        "shard: {}, clk: {}, op: {:?}, bn_p: {:?}, bn_q: {:?}, r: {:?}",
        rt.current_shard(),
        rt.clk,
        op,
        bn_p,
        bn_q,
        r
    );

    let mut result_words = r.to_u32_digits();
    result_words.resize(nw_per_fe, 0);

    let p_memory_records = rt.mw_slice(p_ptr, &result_words);

    let shard = rt.current_shard();
    FieldArithEvent {
        shard,
        clk: start_clk,
        op,
        p,
        q,
        p_ptr,
        q_ptr,
        p_memory_records,
        q_memory_records,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        runtime::Program,
        runtime::Runtime,
        utils::{run_test, setup_logger, tests::BN254_SCALAR_ARITH_ELF},
    };

    #[test]
    fn test_bn254_scalar_arith_simple() {
        setup_logger();
        let program = Program::from(BN254_SCALAR_ARITH_ELF);
        let mut rt = Runtime::new(program);
        rt.run();
        // run_test(program).unwrap();
    }
}
