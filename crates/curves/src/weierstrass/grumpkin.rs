use generic_array::GenericArray;
use num::{BigUint, Num, Zero};
use serde::{Deserialize, Serialize};
use typenum::{U32, U62};

use super::{FieldType, FpOpField, SwCurve, WeierstrassParameters};
use crate::{
    params::{FieldParameters, NumLimbs},
    CurveType, EllipticCurveParameters,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
/// Grumpkin curve parameter
pub struct GrumpkinParameters;

pub type Grumpkin = SwCurve<GrumpkinParameters>;

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
/// Grumpkin base field parameter
pub struct GrumpkinBaseField;

impl FieldParameters for GrumpkinBaseField {
    // python:
    //   fp = 21888242871839275222246405745257275088548364400416034343698204186575808495617
    //   list(int.to_bytes(fp, length=32, byteorder='little'))
    const MODULUS: &'static [u8] = &[
        1, 0, 0, 240, 147, 245, 225, 67, 145, 112, 185, 121, 72, 232, 51, 40, 93, 88, 129, 129,
        182, 69, 80, 184, 41, 160, 49, 225, 114, 78, 100, 48,
    ];

    // A rough witness-offset estimate given the size of the limbs and the size of the field.
    const WITNESS_OFFSET: usize = 1usize << 14;

    // Grumpkin::Fp is Bn254::Fr
    fn modulus() -> BigUint {
        BigUint::from_str_radix(
            "21888242871839275222246405745257275088548364400416034343698204186575808495617",
            10,
        )
        .unwrap()
    }
}

impl FpOpField for GrumpkinBaseField {
    const FIELD_TYPE: FieldType = FieldType::Grumpkin;
}

impl NumLimbs for GrumpkinBaseField {
    type Limbs = U32;
    type Witness = U62;
}

impl EllipticCurveParameters for GrumpkinParameters {
    type BaseField = GrumpkinBaseField;

    const CURVE_TYPE: CurveType = CurveType::Grumpkin;
}

// https://github.com/lambdaclass/lambdaworks/blob/main/math/src/elliptic_curve/short_weierstrass/curves/grumpkin/curve.rs
impl WeierstrassParameters for GrumpkinParameters {
    const A: GenericArray<u8, U32> = GenericArray::from_array([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ]);

    // -17
    // b = fp - 17
    // list(int.to_bytes(b, length=32, byteorder='little'))
    const B: GenericArray<u8, U32> = GenericArray::from_array([
        240, 255, 255, 239, 147, 245, 225, 67, 145, 112, 185, 121, 72, 232, 51, 40, 93, 88, 129,
        129, 182, 69, 80, 184, 41, 160, 49, 225, 114, 78, 100, 48,
    ]);

    fn generator() -> (BigUint, BigUint) {
        let x = BigUint::from(1u32);
        // sqrt(-16)
        let y = BigUint::from_str_radix(
            "17631683881184975370165255887551781615748388533673675138860",
            10,
        )
        .unwrap();
        (x, y)
    }

    fn prime_group_order() -> num::BigUint {
        BigUint::from_str_radix(
            "21888242871839275222246405745257275088696311157297823662689037894645226208583",
            10,
        )
        .unwrap()
    }

    fn a_int() -> BigUint {
        BigUint::zero()
    }

    fn b_int() -> BigUint {
        BigUint::from_str_radix(
            "21888242871839275222246405745257275088548364400416034343698204186575808495600",
            10,
        )
        .unwrap()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::utils::biguint_from_limbs;

    #[test]
    fn test_weierstrass_biguint_scalar_mul() {
        assert_eq!(biguint_from_limbs(GrumpkinBaseField::MODULUS), GrumpkinBaseField::modulus());
    }
}
