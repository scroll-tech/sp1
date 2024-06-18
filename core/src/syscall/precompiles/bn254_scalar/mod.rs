mod mac;
// mod general_field_op;

pub use mac::Bn254ScalarMacChip;

use crate::{
    operations::field::params::Limbs,
    operations::field::params::{FieldParameters, NumWords},
    runtime::{MemoryReadRecord, MemoryWriteRecord, SyscallContext},
    utils::ec::weierstrass::bn254::Bn254ScalarField,
};
use num::BigUint;
use typenum::Unsigned;

use serde::{Deserialize, Serialize};

pub(crate) const NUM_WORDS_PER_FE: usize = 8;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldArithMemoryAccess<T> {
    pub ptr: u32,
    pub memory_records: Vec<T>,
}

impl FieldArithMemoryAccess<MemoryReadRecord> {
    pub fn read(rt: &mut SyscallContext, ptr: u32, len: usize) -> Self {
        let (memory_records, _) = rt.mr_slice(ptr, len);
        Self {
            ptr,
            memory_records,
        }
    }

    pub fn value_as_biguint(&self) -> BigUint {
        BigUint::from_bytes_le(
            &self
                .memory_records
                .iter()
                .flat_map(|word| word.value.to_le_bytes())
                .collect::<Vec<u8>>(),
        )
    }
}

impl FieldArithMemoryAccess<MemoryWriteRecord> {
    pub fn write(rt: &mut SyscallContext, ptr: u32, values: &[u32]) -> Self {
        Self {
            ptr,
            memory_records: rt.mw_slice(ptr, &values),
        }
    }

    pub fn prev_value_as_biguint(&self) -> BigUint {
        BigUint::from_bytes_le(
            &self
                .memory_records
                .iter()
                .flat_map(|word| word.prev_value.to_le_bytes())
                .collect::<Vec<u8>>(),
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bn254FieldArithEvent {
    pub shard: u32,
    pub channel: u32,
    pub clk: u32,
    pub arg1: FieldArithMemoryAccess<MemoryWriteRecord>,
    pub arg2: FieldArithMemoryAccess<MemoryReadRecord>,
    pub a: FieldArithMemoryAccess<MemoryReadRecord>,
    pub b: FieldArithMemoryAccess<MemoryReadRecord>,
    pub c: FieldArithMemoryAccess<MemoryReadRecord>,
}

pub fn create_bn254_scalar_arith_event(
    rt: &mut SyscallContext,
    arg1: u32,
    arg2: u32,
) -> Bn254FieldArithEvent {
    let start_clk = rt.clk;
    let p_ptr = arg1;
    let q_ptr = arg2;

    assert_eq!(p_ptr % 4, 0, "p_ptr({p_ptr:x}) is not aligned");
    assert_eq!(q_ptr % 4, 0, "q_ptr({q_ptr:x}) is not aligned");

    let nw_per_fe = <Bn254ScalarField as NumWords>::WordsFieldElement::USIZE;
    debug_assert_eq!(nw_per_fe, NUM_WORDS_PER_FE);

    let arg1: Vec<u32> = rt.slice_unsafe(p_ptr, nw_per_fe);
    let arg2 = FieldArithMemoryAccess::read(rt, arg2, 3);

    for (idx, ptr) in arg2.memory_records.iter().enumerate() {
        assert_eq!(
            ptr.value % 4,
            0,
            "arg2[{idx}]({:x}) is not aligned",
            ptr.value
        );
    }

    let bn_arg1 = BigUint::from_bytes_le(
        &arg1
            .iter()
            .copied()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<u8>>(),
    );
    let modulus = Bn254ScalarField::modulus();

    let a = FieldArithMemoryAccess::read(rt, arg2.memory_records[0].value, nw_per_fe);
    let b = FieldArithMemoryAccess::read(rt, arg2.memory_records[1].value, nw_per_fe);
    let c = FieldArithMemoryAccess::read(rt, arg2.memory_records[2].value, nw_per_fe);

    let bn_a = a.value_as_biguint();
    let bn_b = b.value_as_biguint();
    let bn_c = c.value_as_biguint();
    let bn_arg1_out = (&bn_a * &bn_b + &bn_arg1 * &bn_c) % modulus;

    log::trace!(
        "shard: {}, clk: {}, arg1: {arg1:?}, arg2: {arg2:?}, a: {a:?}, b: {b:?}, c: {c:?}",
        rt.current_shard(),
        rt.clk,
    );

    let mut result_words = bn_arg1_out.to_u32_digits();
    result_words.resize(nw_per_fe, 0);

    let arg1 = FieldArithMemoryAccess::write(rt, p_ptr, &result_words);

    let shard = rt.current_shard();
    Bn254FieldArithEvent {
        shard,
        channel: rt.current_channel(),
        clk: start_clk,
        arg1,
        arg2,
        a,
        b,
        c,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        runtime::{Program, Runtime},
        utils::{run_test, setup_logger, tests::BN254_SCALAR_ARITH_ELF, SP1CoreOpts},
    };

    #[test]
    fn test_bn254_scalar_arith_simple() {
        setup_logger();
        let program = Program::from(BN254_SCALAR_ARITH_ELF);
        let mut rt = Runtime::new(program, SP1CoreOpts::default());
        rt.run();
        // run_test(program).unwrap();
    }
}
