use num::BigUint;
use sp1_derive::AlignedBorrow;

use crate::operations::field::field_op::FieldOperation;
use crate::operations::field::util_air::eval_field_operation;
use crate::{
    air::Polynomial, operations::field::field_op::FieldOpCols, stark::SP1AirBuilder,
    utils::ec::field::FieldParameters,
};
use p3_field::AbstractField;
use p3_field::PrimeField32;

#[derive(Debug, Clone, AlignedBorrow)]
pub struct GeneralFieldOpCols<T, P: FieldParameters> {
    pub is_sub_div: T,
    pub is_mul_div: T,
    pub cols: FieldOpCols<T, P>,
}

impl<F: PrimeField32, P: FieldParameters> GeneralFieldOpCols<F, P> {
    pub fn populate(&mut self, a: &BigUint, b: &BigUint, op: FieldOperation) -> BigUint {
        let (is_mul_div, is_sub_div) = match op {
            FieldOperation::Add => (0, 0),
            FieldOperation::Sub => (0, 1),
            FieldOperation::Mul => (1, 0),
            FieldOperation::Div => (1, 1),
        };
        self.is_mul_div = F::from_canonical_u32(is_mul_div);
        self.is_sub_div = F::from_canonical_u32(is_sub_div);
        self.cols.populate(a, b, op)
    }
}
impl<V: Copy, P: FieldParameters> GeneralFieldOpCols<V, P> {
    pub fn eval<
        AB: SP1AirBuilder<Var = V>,
        A: Into<Polynomial<AB::Expr>> + Clone,
        B: Into<Polynomial<AB::Expr>> + Clone,
        OP: Into<AB::Expr>,
    >(
        &self,
        builder: &mut AB,
        a: &A,
        b: &B,
        op: OP,
    ) where
        V: Into<AB::Expr>,
    {
        let one = AB::Expr::from(AB::F::one());
        let is_sub_div: AB::Expr = self.is_sub_div.into();
        let is_mul_div: AB::Expr = self.is_mul_div.into();
        let not_sub_div = one.clone() - is_sub_div.clone();
        let not_mul_div = one - is_mul_div.clone();
        builder.assert_bool(is_sub_div.clone());
        builder.assert_bool(is_mul_div.clone());

        // mul: 1 0
        // div: 1 1
        // add: 0 0
        // sub: 0 1
        let assigned_op: AB::Expr = AB::Expr::from(AB::F::from_canonical_u8(0b01))
            * is_sub_div.clone()
            + AB::Expr::from(AB::F::from_canonical_u8(0b10)) * is_mul_div.clone();
        builder.assert_eq(assigned_op, op.into());

        let p_a_param: Polynomial<AB::Expr> = (*a).clone().into();
        let p_b: Polynomial<AB::Expr> = (*b).clone().into();

        let result: Polynomial<AB::Expr> = self.cols.result.clone().into();
        let p_a = &result * is_sub_div.clone() + &p_a_param * not_sub_div.clone();
        let p_result = &p_a_param * is_sub_div.clone() + &result * not_sub_div.clone();
        let p_carry: Polynomial<AB::Expr> = self.cols.carry.clone().into();
        let p_op = &p_a * &p_b * is_mul_div.clone() + (&p_a + &p_b) * not_mul_div;

        let p_op_minus_result: Polynomial<AB::Expr> = p_op - p_result;
        let p_limbs = Polynomial::from_iter(P::modulus_field_iter::<AB::F>().map(AB::Expr::from));
        let p_vanishing = p_op_minus_result - &(&p_carry * &p_limbs);
        let p_witness_low = self.cols.witness_low.0.iter().into();
        let p_witness_high = self.cols.witness_high.0.iter().into();
        eval_field_operation::<AB, P>(builder, &p_vanishing, &p_witness_low, &p_witness_high);
    }
}
