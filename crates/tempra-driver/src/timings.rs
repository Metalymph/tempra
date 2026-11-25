//! Opt-in wall-clock timings for fresh single-source VM compilation.
//!
//! Timing is observational. A missing phase means it was not executed,
//! not that the phase took zero time. Cache reuse and module graphs
//! are reported through opt-in instrumented paths (K5.2b).

use std::time::{Duration, Instant};

/// Compiler phase identity; names are versioned in the CLI JSON schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationPhase {
    Parse,
    HirLower,
    Typecheck,
    TirLower,
    TirVerify,
    EffectsAnalysis,
    DenyEnforcement,
    BytecodeLower,
    BytecodeVerify,
    CacheWrite,
    CacheLookup,
    ModuleGraph,
    ModuleTypecheck,
}

impl CompilationPhase {
    /// Stable machine-readable name for a phase.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse_including_lex",
            Self::HirLower => "hir_lower",
            Self::Typecheck => "typecheck",
            Self::TirLower => "tir_lower",
            Self::TirVerify => "tir_verify",
            Self::EffectsAnalysis => "effects_analysis",
            Self::DenyEnforcement => "deny_enforcement",
            Self::BytecodeLower => "bytecode_lower",
            Self::BytecodeVerify => "bytecode_verify",
            Self::CacheWrite => "cache_write",
            Self::CacheLookup => "cache_lookup",
            Self::ModuleGraph => "module_graph",
            Self::ModuleTypecheck => "module_typecheck",
        }
    }
}

/// A completed attempt at one phase (including attempts ending in failure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseSample {
    pub phase: CompilationPhase,
    pub elapsed_ns: u64,
    pub success: bool,
}

/// Result of one opt-in compilation measurement.
///
/// `wall_ns` covers total API execution, including work not assigned to a
/// phase. Do not sum samples and present the result as process startup time.
#[derive(Debug, Clone, Default)]
pub struct CompilationTimings {
    pub wall_ns: u64,
    pub samples: Vec<PhaseSample>,
    /// Cache reuse decision, present only for a cache-aware invocation.
    pub cache_hit: Option<bool>,
}

pub(crate) fn elapsed_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

/// Run a phase without calling the clock when measurement is disabled.
pub(crate) fn measure<T, E>(
    sink: &mut Option<&mut CompilationTimings>,
    phase: CompilationPhase,
    operation: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    let Some(report) = sink.as_deref_mut() else {
        return operation();
    };
    let started = Instant::now();
    let result = operation();
    report.samples.push(PhaseSample {
        phase,
        elapsed_ns: elapsed_ns(started.elapsed()),
        success: result.is_ok(),
    });
    result
}
