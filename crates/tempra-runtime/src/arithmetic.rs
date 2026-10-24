//! Shared, host-safe arithmetic semantics for Tempra runtime backends.

use std::fmt;

/// A controlled failure produced by Tempra `Int64` arithmetic.
///
/// Every runtime backend must preserve these checked semantics instead of
/// inheriting Rust build-profile behavior or native trap behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Int64ArithmeticError {
    /// The exact signed result is outside the `Int64` range.
    Overflow,
    /// Integer division was attempted with a zero divisor.
    DivisionByZero,
}

impl fmt::Display for Int64ArithmeticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Overflow => "integer overflow",
            Self::DivisionByZero => "integer division by zero",
        })
    }
}

impl std::error::Error for Int64ArithmeticError {}

/// A controlled failure produced by Tempra `UInt64` arithmetic.
///
/// `Underflow` remains distinct from `Overflow`: both are outside the
/// representable unsigned range, but they arise from opposite directions and
/// must not be collapsed by a backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UInt64ArithmeticError {
    /// The exact unsigned result is above `UInt64::MAX`.
    Overflow,
    /// The exact unsigned result is below zero.
    Underflow,
    /// Integer division was attempted with a zero divisor.
    DivisionByZero,
}

impl fmt::Display for UInt64ArithmeticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Overflow => "unsigned integer overflow",
            Self::Underflow => "unsigned integer underflow",
            Self::DivisionByZero => "integer division by zero",
        })
    }
}

impl std::error::Error for UInt64ArithmeticError {}

/// Adds two `UInt64` values using Tempra's checked runtime semantics.
pub fn add_u64(left: u64, right: u64) -> Result<u64, UInt64ArithmeticError> {
    left.checked_add(right)
        .ok_or(UInt64ArithmeticError::Overflow)
}

/// Subtracts two `UInt64` values using Tempra's checked runtime semantics.
pub fn sub_u64(left: u64, right: u64) -> Result<u64, UInt64ArithmeticError> {
    left.checked_sub(right)
        .ok_or(UInt64ArithmeticError::Underflow)
}

/// Multiplies two `UInt64` values using Tempra's checked runtime semantics.
pub fn mul_u64(left: u64, right: u64) -> Result<u64, UInt64ArithmeticError> {
    left.checked_mul(right)
        .ok_or(UInt64ArithmeticError::Overflow)
}

/// Divides two `UInt64` values using Tempra's checked runtime semantics.
pub fn div_u64(left: u64, right: u64) -> Result<u64, UInt64ArithmeticError> {
    if right == 0 {
        return Err(UInt64ArithmeticError::DivisionByZero);
    }
    left.checked_div(right)
        .ok_or(UInt64ArithmeticError::Overflow)
}

/// Adds two `Int64` values using Tempra's checked runtime semantics.
pub fn add_i64(left: i64, right: i64) -> Result<i64, Int64ArithmeticError> {
    left.checked_add(right)
        .ok_or(Int64ArithmeticError::Overflow)
}

/// Subtracts two `Int64` values using Tempra's checked runtime semantics.
pub fn sub_i64(left: i64, right: i64) -> Result<i64, Int64ArithmeticError> {
    left.checked_sub(right)
        .ok_or(Int64ArithmeticError::Overflow)
}

/// Multiplies two `Int64` values using Tempra's checked runtime semantics.
pub fn mul_i64(left: i64, right: i64) -> Result<i64, Int64ArithmeticError> {
    left.checked_mul(right)
        .ok_or(Int64ArithmeticError::Overflow)
}

/// Divides two `Int64` values using Tempra's checked runtime semantics.
///
/// Division by zero remains distinct from the signed overflow case
/// `Int64::MIN / -1`.
pub fn div_i64(left: i64, right: i64) -> Result<i64, Int64ArithmeticError> {
    if right == 0 {
        return Err(Int64ArithmeticError::DivisionByZero);
    }
    left.checked_div(right)
        .ok_or(Int64ArithmeticError::Overflow)
}

/// Negates an `Int64` value using Tempra's checked runtime semantics.
pub fn neg_i64(value: i64) -> Result<i64, Int64ArithmeticError> {
    value.checked_neg().ok_or(Int64ArithmeticError::Overflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_operations_return_exact_values() {
        assert_eq!(add_i64(1, 2), Ok(3));
        assert_eq!(sub_i64(3, 2), Ok(1));
        assert_eq!(mul_i64(3, 2), Ok(6));
        assert_eq!(div_i64(6, 2), Ok(3));
        assert_eq!(neg_i64(3), Ok(-3));
    }

    #[test]
    fn all_signed_overflow_uses_one_category() {
        assert_eq!(add_i64(i64::MAX, 1), Err(Int64ArithmeticError::Overflow));
        assert_eq!(sub_i64(i64::MIN, 1), Err(Int64ArithmeticError::Overflow));
        assert_eq!(mul_i64(i64::MAX, 2), Err(Int64ArithmeticError::Overflow));
        assert_eq!(div_i64(i64::MIN, -1), Err(Int64ArithmeticError::Overflow));
        assert_eq!(neg_i64(i64::MIN), Err(Int64ArithmeticError::Overflow));
    }

    #[test]
    fn division_by_zero_remains_distinct() {
        assert_eq!(div_i64(1, 0), Err(Int64ArithmeticError::DivisionByZero));
    }

    #[test]
    fn unsigned_valid_operations_return_exact_values() {
        assert_eq!(add_u64(1, 2), Ok(3));
        assert_eq!(sub_u64(3, 2), Ok(1));
        assert_eq!(mul_u64(3, 2), Ok(6));
        assert_eq!(div_u64(7, 2), Ok(3));
        assert_eq!(add_u64(0, u64::MAX), Ok(u64::MAX));
        assert_eq!(sub_u64(u64::MAX, u64::MAX), Ok(0));
        assert_eq!(div_u64(u64::MAX, 1), Ok(u64::MAX));
    }

    #[test]
    fn unsigned_overflow_and_underflow_remain_distinct() {
        assert_eq!(add_u64(u64::MAX, 1), Err(UInt64ArithmeticError::Overflow));
        assert_eq!(mul_u64(u64::MAX, 2), Err(UInt64ArithmeticError::Overflow));
        assert_eq!(sub_u64(0, 1), Err(UInt64ArithmeticError::Underflow));
    }

    #[test]
    fn unsigned_division_by_zero_remains_distinct() {
        assert_eq!(div_u64(1, 0), Err(UInt64ArithmeticError::DivisionByZero));
    }
}
