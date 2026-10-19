use crate::externs::capabilities_for_extern;
use crate::{Capability, CapabilitySet};
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::fmt;
use tempra_tir::ir::{
    TirBlockId, TirConst, TirFunction, TirFunctionId, TirInstr, TirKey, TirModule, TirSlotId,
    TirType, TirValueId,
};

/// A stable structural position within a TIR function body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TirLocation {
    /// Function containing the instruction.
    pub function: TirFunctionId,
    /// Function-scoped block containing the instruction.
    pub block: TirBlockId,
    /// Zero-based instruction index within the block.
    pub instruction_index: usize,
}

/// A call observed while scanning a TIR function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallEdge {
    /// Calling function.
    pub caller: TirFunctionId,
    /// Called function or extern declaration.
    pub callee: TirFunctionId,
    /// Structural location of the call instruction.
    pub callsite: TirLocation,
}

/// The operation that directly introduced a capability requirement.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DirectCapabilitySource {
    /// A string constant materialized by the current runtime representation.
    StringConstant,
    /// A TIR `Make` instruction.
    Make,
    /// A TIR `MakeStruct` instruction.
    MakeStruct,
    /// A potentially failing division.
    Division,
    /// A potentially overflowing `Int64` addition.
    IntegerAddition,
    /// A potentially overflowing `Int64` subtraction or lowered negation.
    IntegerSubtraction,
    /// A potentially overflowing `Int64` multiplication.
    IntegerMultiplication,
    /// A potentially failing enum payload unwrap.
    Unwrap,
    /// A constant-index `Get` operation.
    ConstantIndexGet,
    /// A dynamic-index `GetIndex` operation.
    DynamicIndexGet,
    /// A potentially failing slice operation.
    Slice,
    /// Materialization of a string's byte view in the current runtime.
    Bytes,
    /// Construction of a new array by `ArrayPush`.
    ArrayPush,
    /// A call to a compiler-known extern function.
    ExternCall {
        /// TIR ID of the extern declaration.
        extern_id: TirFunctionId,
        /// Fully qualified extern name.
        name: String,
    },
}

/// A direct capability requirement and its structural provenance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DirectCapabilityOrigin {
    /// Capability introduced at this origin.
    pub capability: Capability,
    /// TIR position that introduced the requirement.
    pub location: TirLocation,
    /// Semantic kind of the direct operation.
    pub source: DirectCapabilitySource,
}

/// Direct and transitively required capabilities for one defined function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionCapabilitySummary {
    /// Function described by this summary.
    pub function: TirFunctionId,
    /// Capabilities introduced directly by the function body.
    pub direct: CapabilitySet,
    /// Direct capabilities plus requirements propagated from callees.
    pub required: CapabilitySet,
}

/// One deterministic explanation for a function capability requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityWitness {
    /// Capability explained by this witness.
    pub capability: Capability,
    /// Ordered internal calls from the queried function to the origin function.
    pub calls: Vec<CallEdge>,
    /// Direct operation at the end of the witness path.
    pub origin: DirectCapabilityOrigin,
}

/// Structured failures detected by capability analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityAnalysisError {
    /// Two module functions use the same function ID.
    DuplicateFunctionId {
        /// Duplicated function ID.
        function: TirFunctionId,
    },
    /// Two blocks in the same function use the same block ID.
    DuplicateBlockId {
        /// Function containing the duplicated block ID.
        function: TirFunctionId,
        /// Duplicated function-scoped block ID.
        block: TirBlockId,
    },
    /// A call targets a function ID absent from the module.
    UnknownCallTarget {
        /// Calling function.
        caller: TirFunctionId,
        /// Missing target function ID.
        target: TirFunctionId,
        /// Location of the malformed call.
        callsite: TirLocation,
    },
    /// A call targets a non-extern declaration with no function body.
    UnresolvedDeclaration {
        /// Calling function.
        caller: TirFunctionId,
        /// Declaration function ID.
        target: TirFunctionId,
        /// Declaration name.
        name: String,
        /// Location of the call.
        callsite: TirLocation,
    },
    /// A called extern has no compiler-known capability classification.
    UnclassifiedExtern {
        /// Calling function.
        caller: TirFunctionId,
        /// TIR ID of the extern declaration.
        extern_id: TirFunctionId,
        /// Fully qualified extern name.
        name: String,
        /// Location of the extern call.
        callsite: TirLocation,
    },
}

impl fmt::Display for CapabilityAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateFunctionId { function } => {
                write!(formatter, "duplicate TIR function ID {}", function.0)
            }
            Self::DuplicateBlockId { function, block } => write!(
                formatter,
                "duplicate TIR block ID {} in function {}",
                block.0, function.0
            ),
            Self::UnknownCallTarget { caller, target, .. } => write!(
                formatter,
                "function {} calls unknown TIR function {}",
                caller.0, target.0
            ),
            Self::UnresolvedDeclaration {
                caller,
                target,
                name,
                ..
            } => write!(
                formatter,
                "function {} calls unresolved declaration '{}' ({})",
                caller.0, name, target.0
            ),
            Self::UnclassifiedExtern {
                caller,
                extern_id,
                name,
                ..
            } => write!(
                formatter,
                "function {} calls unclassified extern '{}' ({})",
                caller.0, name, extern_id.0
            ),
        }
    }
}

impl Error for CapabilityAnalysisError {}

/// Complete capability facts derived from one TIR module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityAnalysis {
    summaries: Vec<FunctionCapabilitySummary>,
    call_edges: Vec<CallEdge>,
    direct_origins: Vec<DirectCapabilityOrigin>,
    summary_indices: HashMap<TirFunctionId, usize>,
}

impl CapabilityAnalysis {
    /// Returns all defined-function summaries in ascending function-ID order.
    pub fn summaries(&self) -> &[FunctionCapabilitySummary] {
        &self.summaries
    }

    /// Looks up the summary for a defined function.
    pub fn summary(&self, function: TirFunctionId) -> Option<&FunctionCapabilitySummary> {
        self.summary_indices
            .get(&function)
            .map(|index| &self.summaries[*index])
    }

    /// Returns every observed call edge in deterministic order.
    pub fn call_edges(&self) -> &[CallEdge] {
        &self.call_edges
    }

    /// Iterates over calls made by `function` in deterministic target order.
    pub fn outgoing_calls(&self, function: TirFunctionId) -> impl Iterator<Item = &CallEdge> {
        self.call_edges
            .iter()
            .filter(move |edge| edge.caller == function)
    }

    /// Returns every direct origin in deterministic structural order.
    pub fn direct_origins(&self) -> &[DirectCapabilityOrigin] {
        &self.direct_origins
    }

    /// Iterates over direct origins belonging to `function`.
    pub fn origins_for(
        &self,
        function: TirFunctionId,
    ) -> impl Iterator<Item = &DirectCapabilityOrigin> {
        self.direct_origins
            .iter()
            .filter(move |origin| origin.location.function == function)
    }

    /// Iterates over direct origins for one function and capability.
    pub fn origins_for_capability(
        &self,
        function: TirFunctionId,
        capability: Capability,
    ) -> impl Iterator<Item = &DirectCapabilityOrigin> {
        self.origins_for(function)
            .filter(move |origin| origin.capability == capability)
    }

    /// Reconstructs the canonical shortest witness for a capability requirement.
    pub fn witness(
        &self,
        function: TirFunctionId,
        capability: Capability,
    ) -> Option<CapabilityWitness> {
        if !self.summary(function)?.required.contains(capability) {
            return None;
        }

        let mut queue = VecDeque::from([(function, Vec::new())]);
        let mut visited = HashSet::from([function]);

        while let Some((current, path)) = queue.pop_front() {
            if let Some(origin) = self.origins_for_capability(current, capability).next() {
                return Some(CapabilityWitness {
                    capability,
                    calls: path,
                    origin: origin.clone(),
                });
            }

            for edge in self.outgoing_calls(current) {
                let Some(summary) = self.summary(edge.callee) else {
                    continue;
                };
                if !summary.required.contains(capability) || !visited.insert(edge.callee) {
                    continue;
                }

                let mut next_path = path.clone();
                next_path.push(*edge);
                queue.push_back((edge.callee, next_path));
            }
        }

        None
    }
}

#[derive(Debug, Clone, Copy)]
struct SummaryState {
    function: TirFunctionId,
    direct: CapabilitySet,
    required: CapabilitySet,
}

/// Analyzes an already verified TIR module without mutating it.
///
/// The analyzer still validates IDs and call topology that can be malformed in
/// hand-built modules. It returns the first such error in deterministic scan
/// order instead of panicking.
pub fn analyze(module: &TirModule) -> Result<CapabilityAnalysis, CapabilityAnalysisError> {
    let mut functions: Vec<&TirFunction> = module.functions.iter().collect();
    functions.sort_by_key(|function| function.id.0);

    for pair in functions.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(CapabilityAnalysisError::DuplicateFunctionId {
                function: pair[0].id,
            });
        }
    }

    for function in &functions {
        let mut blocks: Vec<TirBlockId> = function.blocks.iter().map(|block| block.id).collect();
        blocks.sort_by_key(|block| block.0);
        for pair in blocks.windows(2) {
            if pair[0] == pair[1] {
                return Err(CapabilityAnalysisError::DuplicateBlockId {
                    function: function.id,
                    block: pair[0],
                });
            }
        }
    }

    let function_by_id: HashMap<TirFunctionId, &TirFunction> = functions
        .iter()
        .map(|function| (function.id, *function))
        .collect();

    let mut states: Vec<SummaryState> = functions
        .iter()
        .copied()
        .filter(|function| !function.is_extern && !function.blocks.is_empty())
        .map(|function| SummaryState {
            function: function.id,
            direct: CapabilitySet::empty(),
            required: CapabilitySet::empty(),
        })
        .collect();
    let state_indices: HashMap<TirFunctionId, usize> = states
        .iter()
        .enumerate()
        .map(|(index, state)| (state.function, index))
        .collect();

    let mut call_edges = Vec::new();
    let mut internal_edges = Vec::new();
    let mut direct_origins = Vec::new();

    for function in functions
        .iter()
        .copied()
        .filter(|function| !function.is_extern && !function.blocks.is_empty())
    {
        let state_index = state_indices[&function.id];
        let mut value_types: HashMap<TirValueId, TirType> =
            function.params.iter().cloned().collect();
        let mut slot_types: HashMap<TirSlotId, TirType> = HashMap::new();
        let mut blocks: Vec<_> = function.blocks.iter().collect();
        blocks.sort_by_key(|block| block.id.0);

        for block in blocks {
            for (instruction_index, instruction) in block.instrs.iter().enumerate() {
                let location = TirLocation {
                    function: function.id,
                    block: block.id,
                    instruction_index,
                };

                if let Some((capabilities, source)) =
                    classify_instruction(instruction, &value_types)
                {
                    record_origin(
                        &mut states[state_index],
                        &mut direct_origins,
                        capabilities,
                        location,
                        source,
                    );
                }

                record_instruction_type(
                    instruction,
                    &function_by_id,
                    &mut value_types,
                    &mut slot_types,
                );

                let TirInstr::Call(_, target_id, _) = instruction else {
                    continue;
                };

                let Some(target) = function_by_id.get(target_id).copied() else {
                    return Err(CapabilityAnalysisError::UnknownCallTarget {
                        caller: function.id,
                        target: *target_id,
                        callsite: location,
                    });
                };
                let edge = CallEdge {
                    caller: function.id,
                    callee: target.id,
                    callsite: location,
                };
                call_edges.push(edge);

                if target.is_extern {
                    let Some(capabilities) = capabilities_for_extern(&target.name) else {
                        return Err(CapabilityAnalysisError::UnclassifiedExtern {
                            caller: function.id,
                            extern_id: target.id,
                            name: target.name.clone(),
                            callsite: location,
                        });
                    };
                    record_origin(
                        &mut states[state_index],
                        &mut direct_origins,
                        capabilities,
                        location,
                        DirectCapabilitySource::ExternCall {
                            extern_id: target.id,
                            name: target.name.clone(),
                        },
                    );
                } else if target.blocks.is_empty() {
                    return Err(CapabilityAnalysisError::UnresolvedDeclaration {
                        caller: function.id,
                        target: target.id,
                        name: target.name.clone(),
                        callsite: location,
                    });
                } else {
                    internal_edges.push(edge);
                }
            }
        }
    }

    call_edges.sort_by_key(call_edge_order);
    internal_edges.sort_by_key(call_edge_order);
    direct_origins.sort_by_key(origin_order);

    let mut reverse_edges = vec![Vec::new(); states.len()];
    for edge in &internal_edges {
        let caller_index = state_indices[&edge.caller];
        let callee_index = state_indices[&edge.callee];
        reverse_edges[callee_index].push(caller_index);
    }
    for callers in &mut reverse_edges {
        callers.sort_by_key(|index| states[*index].function.0);
        callers.dedup();
    }

    let mut queue: VecDeque<usize> = (0..states.len()).collect();
    let mut queued = vec![true; states.len()];
    while let Some(callee_index) = queue.pop_front() {
        queued[callee_index] = false;
        let callee_required = states[callee_index].required;
        for caller_index in reverse_edges[callee_index].iter().copied() {
            let merged = states[caller_index].required.union(callee_required);
            if merged == states[caller_index].required {
                continue;
            }
            states[caller_index].required = merged;
            if !queued[caller_index] {
                queue.push_back(caller_index);
                queued[caller_index] = true;
            }
        }
    }

    let summaries: Vec<_> = states
        .into_iter()
        .map(|state| FunctionCapabilitySummary {
            function: state.function,
            direct: state.direct,
            required: state.required,
        })
        .collect();
    let summary_indices = summaries
        .iter()
        .enumerate()
        .map(|(index, summary)| (summary.function, index))
        .collect();

    Ok(CapabilityAnalysis {
        summaries,
        call_edges,
        direct_origins,
        summary_indices,
    })
}

fn record_origin(
    state: &mut SummaryState,
    origins: &mut Vec<DirectCapabilityOrigin>,
    capabilities: CapabilitySet,
    location: TirLocation,
    source: DirectCapabilitySource,
) {
    for capability in capabilities.iter() {
        state.direct.insert(capability);
        state.required.insert(capability);
        origins.push(DirectCapabilityOrigin {
            capability,
            location,
            source: source.clone(),
        });
    }
}

fn classify_instruction(
    instruction: &TirInstr,
    value_types: &HashMap<TirValueId, TirType>,
) -> Option<(CapabilitySet, DirectCapabilitySource)> {
    let (capability, source) = match instruction {
        TirInstr::Const(_, TirConst::String(_)) => {
            (Capability::Alloc, DirectCapabilitySource::StringConstant)
        }
        TirInstr::Const(_, _) => return None,
        TirInstr::Make(_, _, _, _) => (Capability::Alloc, DirectCapabilitySource::Make),
        TirInstr::MakeStruct(_, _, _) => (Capability::Alloc, DirectCapabilitySource::MakeStruct),
        TirInstr::Get(_, _, TirKey::Index(_)) => {
            (Capability::Panic, DirectCapabilitySource::ConstantIndexGet)
        }
        TirInstr::Get(_, _, TirKey::Field(_)) | TirInstr::Set(_, _, _, _) => return None,
        TirInstr::Unwrap(_, _, _, _) => (Capability::Panic, DirectCapabilitySource::Unwrap),
        TirInstr::Add(_, operand, _) if value_types.get(operand) == Some(&TirType::Int64) => {
            (Capability::Panic, DirectCapabilitySource::IntegerAddition)
        }
        TirInstr::Sub(_, operand, _) if value_types.get(operand) == Some(&TirType::Int64) => (
            Capability::Panic,
            DirectCapabilitySource::IntegerSubtraction,
        ),
        TirInstr::Mul(_, operand, _) if value_types.get(operand) == Some(&TirType::Int64) => (
            Capability::Panic,
            DirectCapabilitySource::IntegerMultiplication,
        ),
        TirInstr::Div(_, _, _) => (Capability::Panic, DirectCapabilitySource::Division),
        TirInstr::GetIndex(_, _, _) => (Capability::Panic, DirectCapabilitySource::DynamicIndexGet),
        TirInstr::Slice(_, _, _, _) => (Capability::Panic, DirectCapabilitySource::Slice),
        TirInstr::Bytes(_, _) => (Capability::Alloc, DirectCapabilitySource::Bytes),
        TirInstr::ArrayPush(_, _, _) => (Capability::Alloc, DirectCapabilitySource::ArrayPush),
        TirInstr::Tag(_, _)
        | TirInstr::Slot(_, _)
        | TirInstr::Load(_, _)
        | TirInstr::Store(_, _)
        | TirInstr::Call(_, _, _)
        | TirInstr::Add(_, _, _)
        | TirInstr::Sub(_, _, _)
        | TirInstr::Mul(_, _, _)
        | TirInstr::Eq(_, _, _)
        | TirInstr::Lt(_, _, _)
        | TirInstr::Gt(_, _, _)
        | TirInstr::And(_, _, _)
        | TirInstr::Or(_, _, _)
        | TirInstr::Not(_, _)
        | TirInstr::Len(_, _)
        | TirInstr::Size(_, _) => return None,
    };

    Some((CapabilitySet::singleton(capability), source))
}

fn record_instruction_type(
    instruction: &TirInstr,
    functions: &HashMap<TirFunctionId, &TirFunction>,
    value_types: &mut HashMap<TirValueId, TirType>,
    slot_types: &mut HashMap<TirSlotId, TirType>,
) {
    let result = match instruction {
        TirInstr::Const(result, value) => Some((
            *result,
            match value {
                TirConst::Unit => TirType::Unit,
                TirConst::Bool(_) => TirType::Bool,
                TirConst::Int64(_) => TirType::Int64,
                TirConst::UInt64(_) => TirType::UInt64,
                TirConst::Float64(_) => TirType::Float64,
                TirConst::String(_) => TirType::String,
            },
        )),
        TirInstr::Make(result, ty, _, _) => Some((*result, ty.clone())),
        TirInstr::Tag(result, _) => Some((*result, TirType::String)),
        TirInstr::Slot(slot, ty) => {
            slot_types.insert(*slot, ty.clone());
            None
        }
        TirInstr::Load(result, slot) => slot_types.get(slot).cloned().map(|ty| (*result, ty)),
        TirInstr::Call(result, target, _) => functions
            .get(target)
            .map(|function| (*result, function.return_type.clone())),
        TirInstr::Add(result, operand, _)
        | TirInstr::Sub(result, operand, _)
        | TirInstr::Mul(result, operand, _)
        | TirInstr::Div(result, operand, _) => {
            value_types.get(operand).cloned().map(|ty| (*result, ty))
        }
        TirInstr::Eq(result, _, _)
        | TirInstr::Lt(result, _, _)
        | TirInstr::Gt(result, _, _)
        | TirInstr::And(result, _, _)
        | TirInstr::Or(result, _, _)
        | TirInstr::Not(result, _) => Some((*result, TirType::Bool)),
        TirInstr::Len(result, _) | TirInstr::Size(result, _) => Some((*result, TirType::Int64)),
        TirInstr::Bytes(result, _) => Some((*result, TirType::Slice(Box::new(TirType::UInt8)))),
        TirInstr::ArrayPush(result, array, _) => {
            value_types.get(array).cloned().map(|ty| (*result, ty))
        }
        TirInstr::MakeStruct(_, _, _)
        | TirInstr::Get(_, _, _)
        | TirInstr::Set(_, _, _, _)
        | TirInstr::Unwrap(_, _, _, _)
        | TirInstr::Store(_, _)
        | TirInstr::GetIndex(_, _, _)
        | TirInstr::Slice(_, _, _, _) => None,
    };

    if let Some((value, ty)) = result {
        value_types.insert(value, ty);
    }
}

fn call_edge_order(edge: &CallEdge) -> (usize, usize, usize, usize) {
    (
        edge.caller.0,
        edge.callee.0,
        edge.callsite.block.0,
        edge.callsite.instruction_index,
    )
}

fn origin_order(origin: &DirectCapabilityOrigin) -> (usize, usize, usize, u8) {
    (
        origin.location.function.0,
        origin.location.block.0,
        origin.location.instruction_index,
        origin.capability as u8,
    )
}
