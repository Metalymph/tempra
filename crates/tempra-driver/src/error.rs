use std::io;

#[derive(Debug)]
pub enum DriverError {
    IoError(io::Error),
    ParseError(String),
    TypeError(String),
    RuntimeError(String),
    TirLowerError(String),
    /// Structural TIR verification failures, retained before capability analysis.
    TirVerificationError(Vec<tempra_tir::verifier::VerifierError>),
    /// A structural capability analysis failure, not a denied capability.
    CapabilityAnalysisError(tempra_effects::CapabilityAnalysisError),
    /// A user contract could not be bound unambiguously to a concrete TIR function.
    ContractBindingError(Box<crate::ContractBindingError>),
    TirInterpError(tempra_tir_interp::TirInterpError),
    Exit(i32),
    BytecodeSerializeError(String),
    VmError(String),
    CacheIoError(String),
    CacheIncompatible(String),
    UnsupportedExperimentalVm(String),
}

impl From<io::Error> for DriverError {
    fn from(error: io::Error) -> Self {
        DriverError::IoError(error)
    }
}

impl std::fmt::Display for DriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriverError::IoError(e) => write!(f, "IO error: {}", e),
            DriverError::ParseError(e) => write!(f, "Parse error:\n{}", e),
            DriverError::TypeError(e) => write!(f, "Type error:\n{}", e),
            DriverError::RuntimeError(e) => write!(f, "{}", e),
            DriverError::TirLowerError(e) => write!(f, "TIR lowering error:\n{}", e),
            DriverError::TirVerificationError(e) => write!(f, "TIR verification error: {:?}", e),
            DriverError::CapabilityAnalysisError(e) => {
                write!(f, "Capability analysis error: {}", e)
            }
            DriverError::ContractBindingError(e) => write!(f, "Contract binding error: {}", e),
            DriverError::TirInterpError(e) => write!(f, "TIR interpreter error: {}", e),
            DriverError::Exit(code) => write!(f, "Exit signal ({})", code),
            DriverError::BytecodeSerializeError(e) => {
                write!(f, "Bytecode serialization error: {}", e)
            }
            DriverError::VmError(e) => write!(f, "VM error: {}", e),
            DriverError::CacheIoError(e) => write!(f, "Cache IO error: {}", e),
            DriverError::CacheIncompatible(e) => write!(f, "Cache incompatible: {}", e),
            DriverError::UnsupportedExperimentalVm(e) => {
                write!(f, "Unsupported by experimental VM: {}", e)
            }
        }
    }
}
