//! Panic-free integer arithmetic for the shared HIR interpreter kernel.

use std::fmt;

/// A deterministic failure produced by supported integer arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntegerArithmeticError {
    AdditionOverflow,
    SubtractionOverflow,
    MultiplicationOverflow,
    DivisionByZero,
    DivisionOverflow,
    NegationOverflow,
}

impl fmt::Display for IntegerArithmeticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AdditionOverflow => "integer addition overflow",
            Self::SubtractionOverflow => "integer subtraction overflow",
            Self::MultiplicationOverflow => "integer multiplication overflow",
            Self::DivisionByZero => "integer division by zero",
            Self::DivisionOverflow => "integer division overflow",
            Self::NegationOverflow => "integer negation overflow",
        })
    }
}

pub(crate) fn add_i64(left: i64, right: i64) -> Result<i64, IntegerArithmeticError> {
    left.checked_add(right)
        .ok_or(IntegerArithmeticError::AdditionOverflow)
}

pub(crate) fn sub_i64(left: i64, right: i64) -> Result<i64, IntegerArithmeticError> {
    left.checked_sub(right)
        .ok_or(IntegerArithmeticError::SubtractionOverflow)
}

pub(crate) fn mul_i64(left: i64, right: i64) -> Result<i64, IntegerArithmeticError> {
    left.checked_mul(right)
        .ok_or(IntegerArithmeticError::MultiplicationOverflow)
}

pub(crate) fn div_i64(left: i64, right: i64) -> Result<i64, IntegerArithmeticError> {
    if right == 0 {
        return Err(IntegerArithmeticError::DivisionByZero);
    }
    left.checked_div(right)
        .ok_or(IntegerArithmeticError::DivisionOverflow)
}

pub(crate) fn div_u8(left: u8, right: u8) -> Result<u8, IntegerArithmeticError> {
    left.checked_div(right)
        .ok_or(IntegerArithmeticError::DivisionByZero)
}

pub(crate) fn neg_i64(value: i64) -> Result<i64, IntegerArithmeticError> {
    value
        .checked_neg()
        .ok_or(IntegerArithmeticError::NegationOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_integer_arithmetic_succeeds() {
        assert_eq!(add_i64(1, 2), Ok(3));
        assert_eq!(sub_i64(3, 2), Ok(1));
        assert_eq!(mul_i64(3, 2), Ok(6));
        assert_eq!(div_i64(6, 2), Ok(3));
        assert_eq!(div_u8(6, 2), Ok(3));
        assert_eq!(neg_i64(3), Ok(-3));
    }

    #[test]
    fn overflow_categories_are_explicit() {
        assert_eq!(
            add_i64(i64::MAX, 1),
            Err(IntegerArithmeticError::AdditionOverflow)
        );
        assert_eq!(
            sub_i64(i64::MIN, 1),
            Err(IntegerArithmeticError::SubtractionOverflow)
        );
        assert_eq!(
            mul_i64(i64::MAX, 2),
            Err(IntegerArithmeticError::MultiplicationOverflow)
        );
        assert_eq!(
            neg_i64(i64::MIN),
            Err(IntegerArithmeticError::NegationOverflow)
        );
    }

    #[test]
    fn division_failures_are_distinguished() {
        assert_eq!(div_i64(1, 0), Err(IntegerArithmeticError::DivisionByZero));
        assert_eq!(
            div_i64(i64::MIN, -1),
            Err(IntegerArithmeticError::DivisionOverflow)
        );
        assert_eq!(div_u8(1, 0), Err(IntegerArithmeticError::DivisionByZero));
    }
}
