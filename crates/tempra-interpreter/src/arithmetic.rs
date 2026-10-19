//! Interpreter-local arithmetic that is outside the shared `Int64` contract.

use tempra_runtime::Int64ArithmeticError;

pub(crate) fn div_u8(left: u8, right: u8) -> Result<u8, Int64ArithmeticError> {
    left.checked_div(right)
        .ok_or(Int64ArithmeticError::DivisionByZero)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_integer_arithmetic_succeeds() {
        assert_eq!(div_u8(6, 2), Ok(3));
    }

    #[test]
    fn division_failures_are_distinguished() {
        assert_eq!(div_u8(1, 0), Err(Int64ArithmeticError::DivisionByZero));
    }
}
