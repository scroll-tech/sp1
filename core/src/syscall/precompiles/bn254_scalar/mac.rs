use crate::runtime::Syscall;
use crate::syscall::precompiles::bn254_scalar::NUM_WORDS_PER_FE;
use crate::utils::ec::field::{FieldParameters, NumWords};
use crate::utils::ec::weierstrass::bn254::Bn254ScalarField;
use num::BigUint;
use typenum::Unsigned;

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
        let p_ptr = arg1;
        let q_ptr = arg2;

        assert_eq!(p_ptr % 4, 0, "p_ptr({:x}) is not aligned", p_ptr);
        assert_eq!(q_ptr % 4, 0, "q_ptr({:x}) is not aligned", q_ptr);

        let nw_per_fe = <Bn254ScalarField as NumWords>::WordsFieldElement::USIZE;
        debug_assert_eq!(nw_per_fe, NUM_WORDS_PER_FE);

        let ret_in = rt.slice_unsafe(arg1, nw_per_fe);
        let ptr = rt.slice_unsafe(arg2, 2);
        let a = rt.slice_unsafe(ptr[0], nw_per_fe);
        let b = rt.slice_unsafe(ptr[1], nw_per_fe);

        let bn_ret_in = BigUint::from_bytes_le(
            &ret_in
                .iter()
                .copied()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<u8>>(),
        );
        let bn_a = BigUint::from_bytes_le(
            &a.iter()
                .copied()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<u8>>(),
        );
        let bn_b = BigUint::from_bytes_le(
            &b.iter()
                .copied()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<u8>>(),
        );

        let modulus = Bn254ScalarField::modulus();

        let bn_ret_out = ((bn_a * bn_b) % modulus.clone() + bn_ret_in) % modulus;
        let mut result_words = bn_ret_out.to_u32_digits();
        result_words.resize(nw_per_fe, 0);

        let _p_memory_records = rt.mw_slice(p_ptr, &result_words);

        None
    }

    fn num_extra_cycles(&self) -> u32 {
        1
    }
}
