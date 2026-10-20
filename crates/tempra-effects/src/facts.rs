//! Compilation-local specialized-operation facts derived from verified TIR.
//!
//! This module defines conservative local extraction and the fact vocabulary.
//! Call-graph propagation and witnesses live in `analysis` and reuse the
//! existing internal call edges; this module does not build a second call graph,
//! enforce specialized contracts, insert runtime guards, or claim runtime
//! availability. Operation identity and resource knowledge are kept separate so
//! a known operation with an unknown resource is never collapsed into a generic
//! broad capability.

use crate::{Capability, EffectOperation, EffectParameter, TirLocation};
use std::collections::HashMap;
use tempra_tir::ir::{TirBlockId, TirConst, TirFunction, TirFunctionId, TirInstr, TirValueId};

/// A typed, statically known resource value.
///
/// Only values proved from a dominating [`TirInstr::Const`] definition are
/// represented here. Anything else is [`ResourceKnowledge::Unknown`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceValue {
    /// An exact text spelling, such as a filesystem path compared as written.
    Text(String),
    /// An exact numeric port.
    Port(u16),
}

/// Resource knowledge attached to one specialized operation fact.
///
/// This mirrors the two known-operation states of the v0.1 model. The third
/// state, an unknown operation under a known capability, is represented by
/// [`UnclassifiedOperationFact`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceKnowledge {
    /// The operation's resource was proved exactly at the call site.
    Exact(ResourceValue),
    /// The operation is identified but its resource could not be proved.
    ///
    /// This is fail-closed metadata, not permission.
    Unknown,
}

/// One parameter of a specialized operation and its resource knowledge.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceFact {
    /// Parameter key bound by the native extern classification.
    pub parameter: EffectParameter,
    /// Exact or unknown knowledge for that parameter.
    pub knowledge: ResourceKnowledge,
}

/// A call to a known extern with an explicitly classified specialized operation.
///
/// The fact's origin is one call instruction. Per-function summaries may carry
/// the same fact transitively without changing its original [`TirLocation`].
/// Holding this fact does not mean the call is allowed, enforced, or reachable at
/// runtime.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpecializedOperationFact {
    /// Specialized operation identity, preserved even when resources are unknown.
    pub operation: EffectOperation,
    /// Every declared parameter exactly once, in [`EffectParameter::ALL`] order.
    ///
    /// Parameters whose value could not be proved are present with
    /// [`ResourceKnowledge::Unknown`]; they are never silently omitted.
    pub resources: Vec<ResourceFact>,
    /// Structural location of the call instruction.
    pub location: TirLocation,
    /// TIR ID of the extern declaration.
    pub extern_id: TirFunctionId,
    /// Fully qualified extern name.
    pub extern_name: String,
}

/// A known extern capability requirement with no specialized operation.
///
/// This is the explicit "unknown operation under a known parent capability"
/// state. It is deliberately a different variant from
/// [`SpecializedOperationFact`]; an unknown operation is never assumed to be a
/// different sibling.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnclassifiedOperationFact {
    /// Broad parent capability required by the unrefined operation.
    pub capability: Capability,
    /// Structural location of the call instruction.
    pub location: TirLocation,
    /// TIR ID of the extern declaration.
    pub extern_id: TirFunctionId,
    /// Fully qualified extern name.
    pub extern_name: String,
}

/// One local specialized-effect fact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LocalEffectFact {
    /// A known operation with known or unknown resource knowledge.
    Specialized(SpecializedOperationFact),
    /// A known capability requirement whose operation has no classification.
    UnclassifiedOperation(UnclassifiedOperationFact),
}

impl LocalEffectFact {
    /// Returns the structural location of the fact.
    pub const fn location(&self) -> TirLocation {
        match self {
            Self::Specialized(fact) => fact.location,
            Self::UnclassifiedOperation(fact) => fact.location,
        }
    }
}

/// Index of every value definition in one function body.
///
/// The index exists to make local constant resolution conservative and
/// deterministic. Exact values are recognized only when a value has exactly one
/// definition in the whole function and that definition is a `Const` earlier in
/// the same block as the call. Parameters, loads, stores, calls, other blocks,
/// branches, loops and multiple definitions all degrade to
/// [`ResourceKnowledge::Unknown`].
pub(crate) struct FunctionValueIndex<'a> {
    definitions: HashMap<TirValueId, Vec<ValueDefinition<'a>>>,
}

#[derive(Clone, Copy)]
enum DefinitionSite {
    Parameter,
    Instruction {
        block: TirBlockId,
        instruction_index: usize,
    },
}

struct ValueDefinition<'a> {
    site: DefinitionSite,
    constant: Option<&'a TirConst>,
}

impl<'a> FunctionValueIndex<'a> {
    /// Builds the definition index for `function`.
    pub(crate) fn new(function: &'a TirFunction) -> Self {
        let mut definitions: HashMap<TirValueId, Vec<ValueDefinition<'a>>> = HashMap::new();

        for (value, _) in &function.params {
            definitions
                .entry(*value)
                .or_default()
                .push(ValueDefinition {
                    site: DefinitionSite::Parameter,
                    constant: None,
                });
        }

        for block in &function.blocks {
            for (instruction_index, instruction) in block.instrs.iter().enumerate() {
                let Some((value, constant)) = defined_value(instruction) else {
                    continue;
                };
                definitions.entry(value).or_default().push(ValueDefinition {
                    site: DefinitionSite::Instruction {
                        block: block.id,
                        instruction_index,
                    },
                    constant,
                });
            }
        }

        Self { definitions }
    }

    /// Resolves `argument` as a resource value at `callsite`.
    ///
    /// The value must have exactly one definition, that definition must be a
    /// constant in the same block and strictly before the call. Otherwise this
    /// returns [`ResourceKnowledge::Unknown`].
    pub(crate) fn resolve(&self, argument: TirValueId, callsite: TirLocation) -> ResourceKnowledge {
        let Some(definitions) = self.definitions.get(&argument) else {
            return ResourceKnowledge::Unknown;
        };
        let [definition] = definitions.as_slice() else {
            return ResourceKnowledge::Unknown;
        };
        let DefinitionSite::Instruction {
            block,
            instruction_index,
        } = definition.site
        else {
            return ResourceKnowledge::Unknown;
        };
        if block != callsite.block || instruction_index >= callsite.instruction_index {
            return ResourceKnowledge::Unknown;
        }

        definition
            .constant
            .and_then(exact_resource_value)
            .map(ResourceKnowledge::Exact)
            .unwrap_or(ResourceKnowledge::Unknown)
    }
}

fn defined_value(instruction: &TirInstr) -> Option<(TirValueId, Option<&TirConst>)> {
    match instruction {
        TirInstr::Const(value, constant) => Some((*value, Some(constant))),
        TirInstr::Make(value, _, _, _)
        | TirInstr::MakeStruct(value, _, _)
        | TirInstr::Get(value, _, _)
        | TirInstr::Set(value, _, _, _)
        | TirInstr::Tag(value, _)
        | TirInstr::Unwrap(value, _, _, _)
        | TirInstr::Load(value, _)
        | TirInstr::Call(value, _, _)
        | TirInstr::Add(value, _, _)
        | TirInstr::Sub(value, _, _)
        | TirInstr::Mul(value, _, _)
        | TirInstr::Div(value, _, _)
        | TirInstr::Eq(value, _, _)
        | TirInstr::Lt(value, _, _)
        | TirInstr::Gt(value, _, _)
        | TirInstr::And(value, _, _)
        | TirInstr::Or(value, _, _)
        | TirInstr::Not(value, _)
        | TirInstr::GetIndex(value, _, _)
        | TirInstr::Slice(value, _, _, _)
        | TirInstr::Len(value, _)
        | TirInstr::Size(value, _)
        | TirInstr::Bytes(value, _)
        | TirInstr::ArrayPush(value, _, _) => Some((*value, None)),
        TirInstr::Slot(_, _) | TirInstr::Store(_, _) => None,
    }
}

fn exact_resource_value(constant: &TirConst) -> Option<ResourceValue> {
    match constant {
        TirConst::String(text) => Some(ResourceValue::Text(text.clone())),
        TirConst::Int64(port) => u16::try_from(*port).ok().map(ResourceValue::Port),
        TirConst::UInt64(port) => u16::try_from(*port).ok().map(ResourceValue::Port),
        TirConst::Unit | TirConst::Bool(_) | TirConst::Float64(_) => None,
    }
}
