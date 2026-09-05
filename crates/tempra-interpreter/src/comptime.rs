//! Canonical entrypoint for evaluating typed HIR during comptime.

use crate::control_flow::ControlFlow;
use crate::env::RuntimeEnv;
use crate::interpreter::{EvalMode, Interpreter};
use crate::value::Value;
use std::error::Error;
use std::fmt;
use tempra_hir::HirExpression;

/// The default expression-step budget for one comptime evaluation session.
pub const DEFAULT_COMPTIME_FUEL: u64 = 1_000_000;

pub(crate) const FUEL_EXHAUSTED_MESSAGE: &str = "comptime evaluation exceeded step limit";

/// Effect categories for host-observable operations currently implemented by
/// the HIR interpreter. This is deliberately narrower than the capability
/// vocabulary used by TIR analysis: it describes only operations that can be
/// executed by this evaluator today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComptimeEffect {
    Output,
    Input,
    Filesystem,
    Environment,
    HostOs,
    Process,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ForbiddenComptimeEffect {
    effect: ComptimeEffect,
    operation: String,
}

impl fmt::Display for ForbiddenComptimeEffect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.effect {
            ComptimeEffect::Process => {
                formatter.write_str("command execution is not allowed during comptime evaluation")
            }
            ComptimeEffect::Exit => {
                formatter.write_str("exit is not allowed during comptime evaluation")
            }
            ComptimeEffect::Output
            | ComptimeEffect::Input
            | ComptimeEffect::Filesystem
            | ComptimeEffect::Environment
            | ComptimeEffect::HostOs => write!(
                formatter,
                "operation '{}' is not allowed during comptime evaluation",
                self.operation
            ),
        }
    }
}

pub(crate) fn reject_effect_during_comptime(
    mode: EvalMode,
    effect: ComptimeEffect,
    operation: impl Into<String>,
) -> Result<(), String> {
    if mode == EvalMode::Comptime {
        Err(ForbiddenComptimeEffect {
            effect,
            operation: operation.into(),
        }
        .to_string())
    } else {
        Ok(())
    }
}

/// Configuration for one canonical comptime evaluation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComptimeEvalConfig {
    fuel: u64,
}

impl ComptimeEvalConfig {
    /// Creates a comptime configuration with the given expression-step budget.
    pub const fn with_fuel(fuel: u64) -> Self {
        Self { fuel }
    }

    /// Returns the configured expression-step budget.
    pub const fn fuel(&self) -> u64 {
        self.fuel
    }
}

impl Default for ComptimeEvalConfig {
    fn default() -> Self {
        Self::with_fuel(DEFAULT_COMPTIME_FUEL)
    }
}

/// A normalized failure at the canonical comptime evaluation boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComptimeEvalError {
    /// The configured comptime expression-step budget was exhausted.
    FuelExhausted,
    /// The HIR interpreter rejected an operation while evaluating the expression.
    Evaluation(String),
    /// Evaluation completed with control flow that cannot be a comptime value.
    UnexpectedControlFlow(String),
}

impl fmt::Display for ComptimeEvalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FuelExhausted => formatter.write_str(FUEL_EXHAUSTED_MESSAGE),
            Self::Evaluation(message) => formatter.write_str(message),
            Self::UnexpectedControlFlow(flow) => write!(
                formatter,
                "comptime evaluation produced non-value control flow: {flow}"
            ),
        }
    }
}

impl Error for ComptimeEvalError {}

impl Interpreter {
    /// Evaluates one typed HIR expression in a scoped comptime session.
    ///
    /// The interpreter's previous execution mode and fuel are restored after
    /// success or any ordinary evaluation failure. Host panics are not caught;
    /// the shared arithmetic kernel instead returns deterministic errors for
    /// supported integer overflow and invalid division.
    ///
    /// This method is the outer session boundary. Nested `comptime` expressions
    /// are evaluated by the interpreter kernel inside the active session, so
    /// they retain its remaining fuel instead of opening this facade again.
    pub fn evaluate_comptime(
        &mut self,
        expression: &HirExpression,
        environment: &mut RuntimeEnv,
        config: ComptimeEvalConfig,
    ) -> Result<Value, ComptimeEvalError> {
        let previous_mode = self.mode;
        let previous_fuel = self.fuel;

        self.mode = EvalMode::Comptime;
        self.fuel = Some(config.fuel());
        let result = self.eval_expression(expression, environment);
        self.mode = previous_mode;
        self.fuel = previous_fuel;

        match result {
            Ok(ControlFlow::Value(value)) => Ok(value),
            Ok(flow) => Err(ComptimeEvalError::UnexpectedControlFlow(format!(
                "{flow:?}"
            ))),
            Err(message) if message == FUEL_EXHAUSTED_MESSAGE => {
                Err(ComptimeEvalError::FuelExhausted)
            }
            Err(message) => Err(ComptimeEvalError::Evaluation(message)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempra_hir::{lower_file, HirComptimeExpr, HirExpression, HirFile, HirItem, HirStatement};
    use tempra_parser::parse_source;

    fn interpreter() -> Interpreter {
        Interpreter::new(&HirFile {
            items: Vec::new(),
            top_level_statements: Vec::new(),
        })
    }

    fn expression(source: &str) -> HirExpression {
        let source = format!("fn main() {{ {source} }}");
        let ast = parse_source(&source).unwrap();
        let hir = lower_file(&source, &ast);
        let function = match &hir.items[0] {
            HirItem::Function(function) => function,
            item => panic!("expected function, found {item:?}"),
        };
        let body = function.body.as_ref().unwrap();
        match body.statements.last().unwrap() {
            HirStatement::Expr(expression) => expression.clone(),
            statement => panic!("expected expression statement, found {statement:?}"),
        }
    }

    fn comptime(expression: HirExpression) -> HirExpression {
        let span = expression.span();
        HirExpression::Comptime(HirComptimeExpr {
            span,
            expression: Box::new(expression),
        })
    }

    #[test]
    fn evaluates_a_simple_value() {
        let mut interpreter = interpreter();
        let mut environment = RuntimeEnv::new();

        let value = interpreter
            .evaluate_comptime(&expression("1 + 2"), &mut environment, Default::default())
            .unwrap();

        assert_eq!(value, Value::Int64(3));
    }

    #[test]
    fn installs_comptime_mode() {
        let mut interpreter = interpreter();
        let mut environment = RuntimeEnv::new();

        let error = interpreter
            .evaluate_comptime(
                &expression("os.cwd()"),
                &mut environment,
                Default::default(),
            )
            .unwrap_err();

        assert_eq!(
            error,
            ComptimeEvalError::Evaluation(
                "operation 'os.cwd' is not allowed during comptime evaluation".to_string()
            )
        );
    }

    #[test]
    fn installs_configured_fuel_and_reports_exhaustion() {
        let mut interpreter = interpreter();
        let mut environment = RuntimeEnv::new();

        let error = interpreter
            .evaluate_comptime(
                &expression("1"),
                &mut environment,
                ComptimeEvalConfig::with_fuel(0),
            )
            .unwrap_err();

        assert_eq!(error, ComptimeEvalError::FuelExhausted);
    }

    #[test]
    fn restores_state_after_success() {
        let mut interpreter = interpreter();
        interpreter.mode = EvalMode::Runtime;
        interpreter.fuel = Some(17);
        let mut environment = RuntimeEnv::new();

        let value = interpreter
            .evaluate_comptime(&expression("1"), &mut environment, Default::default())
            .unwrap();

        assert_eq!(value, Value::Int64(1));
        assert_eq!(interpreter.mode, EvalMode::Runtime);
        assert_eq!(interpreter.fuel, Some(17));
    }

    #[test]
    fn restores_state_after_evaluation_failure() {
        let mut interpreter = interpreter();
        interpreter.mode = EvalMode::Runtime;
        interpreter.fuel = Some(19);
        let mut environment = RuntimeEnv::new();

        let error = interpreter
            .evaluate_comptime(&expression("missing"), &mut environment, Default::default())
            .unwrap_err();

        assert_eq!(
            error,
            ComptimeEvalError::Evaluation("Variable missing not found".to_string())
        );
        assert_eq!(interpreter.mode, EvalMode::Runtime);
        assert_eq!(interpreter.fuel, Some(19));
    }

    #[test]
    fn restores_state_after_unexpected_control_flow() {
        let mut interpreter = interpreter();
        interpreter.mode = EvalMode::Runtime;
        interpreter.fuel = Some(23);
        let mut environment = RuntimeEnv::new();
        let expression = expression("{ return 7 }");

        let error = interpreter
            .evaluate_comptime(&expression, &mut environment, Default::default())
            .unwrap_err();

        assert!(matches!(
            error,
            ComptimeEvalError::UnexpectedControlFlow(flow) if flow == "Return(Int64(7))"
        ));
        assert_eq!(interpreter.mode, EvalMode::Runtime);
        assert_eq!(interpreter.fuel, Some(23));
    }

    #[test]
    fn runtime_comptime_node_opens_a_canonical_session() {
        let mut interpreter = interpreter();
        interpreter.mode = EvalMode::Runtime;
        interpreter.fuel = Some(0);
        let mut environment = RuntimeEnv::new();

        let flow = interpreter
            .eval_expression(&comptime(expression("5")), &mut environment)
            .unwrap();

        assert!(matches!(flow, ControlFlow::Value(Value::Int64(5))));
        assert_eq!(interpreter.mode, EvalMode::Runtime);
        assert_eq!(interpreter.fuel, Some(0));
    }

    #[test]
    fn nested_comptime_uses_the_outer_remaining_fuel() {
        let mut interpreter = interpreter();
        let mut environment = RuntimeEnv::new();

        let error = interpreter
            .evaluate_comptime(
                &comptime(expression("5")),
                &mut environment,
                ComptimeEvalConfig::with_fuel(1),
            )
            .unwrap_err();

        assert_eq!(error, ComptimeEvalError::FuelExhausted);
        assert_eq!(interpreter.mode, EvalMode::Runtime);
        assert_eq!(interpreter.fuel, None);
    }
}
