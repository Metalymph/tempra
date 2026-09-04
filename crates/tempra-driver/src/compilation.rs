use crate::{contracts::ContractBindings, DriverError, FunctionCapabilityContract};
use tempra_bytecode::BytecodeModule;
use tempra_effects::CapabilityAnalysis;
use tempra_tir::ir::{TirFunctionId, TirModule};

// Keep the immutable analysis tied to exactly the module that produced it.
#[derive(Debug)]
struct AnalyzedTir {
    module: TirModule,
    capabilities: CapabilityAnalysis,
    contracts: Vec<(TirFunctionId, FunctionCapabilityContract)>,
}

/// A fresh VM compilation with verified TIR, capability facts and verified bytecode.
///
/// Cache-hit execution does not produce this value. Facts describe the retained
/// TIR conservatively. Declaration contracts carry source provenance, but no
/// contracts are enforced and instruction-level source mapping is not provided.
#[derive(Debug)]
pub struct VmCompilation {
    analyzed: AnalyzedTir,
    bytecode: BytecodeModule,
}

impl VmCompilation {
    /// Returns the verified TIR used for capability analysis and bytecode lowering.
    pub fn tir(&self) -> &TirModule {
        &self.analyzed.module
    }

    /// Returns the capability facts computed once for this TIR.
    pub fn capabilities(&self) -> &CapabilityAnalysis {
        &self.analyzed.capabilities
    }

    /// Returns explicit user intent for this compilation's function ID, if any.
    pub fn contract(&self, function: TirFunctionId) -> Option<&FunctionCapabilityContract> {
        self.analyzed
            .contracts
            .binary_search_by_key(&function.0, |(id, _)| id.0)
            .ok()
            .map(|index| &self.analyzed.contracts[index].1)
    }

    /// Iterates explicit contracts in ascending TIR function ID order.
    /// Unannotated functions have no synthetic empty contract.
    pub fn contracts(&self) -> impl Iterator<Item = (TirFunctionId, &FunctionCapabilityContract)> {
        self.analyzed
            .contracts
            .iter()
            .map(|(id, contract)| (*id, contract))
    }

    /// Returns the verified bytecode lowered from the retained TIR.
    pub fn bytecode(&self) -> &BytecodeModule {
        &self.bytecode
    }

    /// Consumes the compilation, retaining only its verified bytecode.
    pub fn into_bytecode(self) -> BytecodeModule {
        self.bytecode
    }
}

pub(crate) fn compile_tir_module(
    module: TirModule,
    bindings: ContractBindings,
) -> Result<VmCompilation, DriverError> {
    tempra_tir::verifier::verify_module(&module).map_err(DriverError::TirVerificationError)?;
    let capabilities =
        tempra_effects::analyze(&module).map_err(DriverError::CapabilityAnalysisError)?;
    let analyzed = AnalyzedTir {
        module,
        capabilities,
        contracts: bindings.finish(),
    };
    let bytecode = tempra_bytecode_lower::lower_tir_module(&analyzed.module).map_err(|e| {
        DriverError::UnsupportedExperimentalVm(format!("bytecode lowering error: {:?}", e))
    })?;
    tempra_bytecode::verify::verify_module(&bytecode).map_err(|e| {
        DriverError::UnsupportedExperimentalVm(format!("verification error: {:?}", e))
    })?;
    Ok(VmCompilation { analyzed, bytecode })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempra_tir::{
        builder::ModuleBuilder,
        ir::{TirConst, TirType},
    };

    #[test]
    fn tir_verification_precedes_capability_analysis() {
        let mut builder = ModuleBuilder::new();
        let mut external = builder.new_function("unclassified", TirType::Int64).build();
        external.is_extern = true;
        let external_id = external.id;
        builder.add_function(external);
        let mut main = builder.new_function("main", TirType::Int64);
        let block = main.new_block();
        main.set_current_block(block);
        main.build_call(external_id, vec![]).unwrap();
        let wrong = main.build_const(TirConst::Bool(true)).unwrap();
        main.build_ret(Some(wrong)).unwrap();
        builder.add_function(main.build());
        assert!(matches!(
            tempra_effects::analyze(&builder.module),
            Err(tempra_effects::CapabilityAnalysisError::UnclassifiedExtern { .. })
        ));
        match compile_tir_module(builder.module, ContractBindings::default()).unwrap_err() {
            DriverError::TirVerificationError(errors) => assert!(errors.contains(
                &tempra_tir::verifier::VerifierError::TypeMismatch {
                    expected: TirType::Int64,
                    actual: TirType::Bool
                }
            )),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
