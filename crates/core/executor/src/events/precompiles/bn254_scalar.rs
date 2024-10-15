use num::BigUint;
use sp1_curves::{
    params::{FieldParameters, NumWords},
    weierstrass::bn254::Bn254ScalarField,
};
use typenum::Unsigned;

use serde::{Deserialize, Serialize};

use crate::{
    events::{LookupId, MemoryLocalEvent, MemoryReadRecord, MemoryWriteRecord},
    syscalls::SyscallContext,
};

use super::FieldOperation;

pub const NUM_WORDS_PER_FE: usize = 8;

#[derive(Default, PartialEq, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum Bn254FieldOperation {
    #[default]
    Invalid = 0,
    Mul = 2,
    Mac = 4,
}

impl Bn254FieldOperation {
    pub const fn to_field_operation(&self) -> FieldOperation {
        match self {
            Bn254FieldOperation::Mul => FieldOperation::Mul,
            Bn254FieldOperation::Mac => panic!("not supported"),
            Bn254FieldOperation::Invalid => panic!("what??"),
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Bn254FieldArithEvent {
    pub lookup_id: LookupId,
    pub shard: u32,
    pub clk: u32,
    pub op: Bn254FieldOperation,
    pub arg1: FieldArithMemoryAccess<MemoryWriteRecord>,
    pub arg2: FieldArithMemoryAccess<MemoryReadRecord>,
    pub a: Option<FieldArithMemoryAccess<MemoryReadRecord>>,
    pub b: Option<FieldArithMemoryAccess<MemoryReadRecord>>,
    /// The local memory access records.
    pub local_mem_access: Vec<MemoryLocalEvent>,
}

pub fn create_bn254_scalar_arith_event(
    rt: &mut SyscallContext,
    arg1: u32,
    arg2: u32,
    op: Bn254FieldOperation,
) -> Bn254FieldArithEvent {
    let start_clk = rt.clk;
    let p_ptr = arg1;
    let q_ptr = arg2;

    assert_eq!(p_ptr % 4, 0, "p_ptr({p_ptr:x}) is not aligned");
    assert_eq!(q_ptr % 4, 0, "q_ptr({q_ptr:x}) is not aligned");

    let nw_per_fe = <Bn254ScalarField as NumWords>::WordsFieldElement::USIZE;
    debug_assert_eq!(nw_per_fe, NUM_WORDS_PER_FE);

    let arg1: Vec<u32> = rt.slice_unsafe(p_ptr, nw_per_fe);
    let arg2 = match op {
        // 2 ptrs of real U256 values
        Bn254FieldOperation::Mac => FieldArithMemoryAccess::read(rt, arg2, 2),
        _ => FieldArithMemoryAccess::read(rt, arg2, nw_per_fe),
    };

    let bn_arg1 = BigUint::from_bytes_le(
        &arg1.iter().copied().flat_map(u32::to_le_bytes).collect::<Vec<u8>>(),
    );
    let modulus = Bn254ScalarField::modulus();

    let (a, b, bn_arg1_out) = if matches!(op, Bn254FieldOperation::Mac) {
        let a = FieldArithMemoryAccess::read(rt, arg2.memory_records[0].value, nw_per_fe);
        let b = FieldArithMemoryAccess::read(rt, arg2.memory_records[1].value, nw_per_fe);

        let bn_a = a.value_as_biguint();
        let bn_b = b.value_as_biguint();
        let bn_arg1_out = (&bn_a * &bn_b + &bn_arg1) % modulus;

        (Some(a), Some(b), bn_arg1_out)
    } else {
        let bn_arg2 = arg2.value_as_biguint();

        let bn_arg1_out = match op {
            Bn254FieldOperation::Mul => (&bn_arg1 * &bn_arg2) % modulus,
            _ => unimplemented!("not supported"),
        };
        (None, None, bn_arg1_out)
    };

    log::trace!(
        "shard: {}, clk: {}, op: {:?}, arg1: {:?}, arg2: {:?}, a: {:?}, b: {:?}",
        rt.current_shard(),
        rt.clk,
        op,
        arg1,
        arg2,
        a,
        b
    );
    rt.clk += 1;

    let mut result_words = bn_arg1_out.to_u32_digits();
    result_words.resize(nw_per_fe, 0);

    let arg1 = FieldArithMemoryAccess::write(rt, p_ptr, &result_words);

    let shard = rt.current_shard();
    Bn254FieldArithEvent {
        lookup_id: rt.syscall_lookup_id,
        shard,
        clk: start_clk,
        op,
        arg1,
        arg2,
        a,
        b,
        local_mem_access: rt.postprocess(),
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct FieldArithMemoryAccess<T> {
    pub ptr: u32,
    pub memory_records: Vec<T>,
}

impl FieldArithMemoryAccess<MemoryReadRecord> {
    pub fn read(rt: &mut SyscallContext, ptr: u32, len: usize) -> Self {
        let (memory_records, _) = rt.mr_slice(ptr, len);
        Self { ptr, memory_records }
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
        Self { ptr, memory_records: rt.mw_slice(ptr, values) }
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
