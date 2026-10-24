use crate::analysis::resource_facts_for_binding;
use crate::externs::{
    capabilities_for_extern, operation_binding_for_extern, operation_for_extern, static_binding,
    BindingError, ExternOperationBinding, ResourceBinding,
};
use crate::{
    analyze, known_externs, Capability, CapabilityAnalysisError, CapabilitySet,
    DirectCapabilitySource, EffectCategory, EffectFamily, EffectOperation, EffectParameter,
    LocalEffectFact, ResourceKnowledge, ResourceValue, SpecializedOperationFact, TirLocation,
};
use std::collections::HashMap;
use tempra_tir::ir::{
    TirBlock, TirBlockId, TirConst, TirFunction, TirFunctionId, TirInstr, TirKey, TirModule,
    TirSlotId, TirTerminator, TirType, TirValueId,
};

fn block(id: usize, instrs: Vec<TirInstr>) -> TirBlock {
    block_with_terminator(id, instrs, TirTerminator::Ret(None))
}

fn block_with_terminator(id: usize, instrs: Vec<TirInstr>, terminator: TirTerminator) -> TirBlock {
    TirBlock {
        id: TirBlockId(id),
        instrs,
        terminator: Some(terminator),
    }
}

fn function(id: usize, name: &str, instrs: Vec<TirInstr>) -> TirFunction {
    function_with_blocks(id, name, vec![block(0, instrs)])
}

fn function_with_blocks(id: usize, name: &str, blocks: Vec<TirBlock>) -> TirFunction {
    TirFunction {
        id: TirFunctionId(id),
        name: name.to_owned(),
        params: Vec::new(),
        return_type: TirType::Unit,
        is_extern: false,
        blocks,
        next_value_id: 0,
        next_block_id: 0,
        next_slot_id: 0,
        debug_slots: HashMap::new(),
    }
}

fn extern_function(id: usize, name: &str) -> TirFunction {
    TirFunction {
        id: TirFunctionId(id),
        name: name.to_owned(),
        params: Vec::new(),
        return_type: TirType::Unit,
        is_extern: true,
        blocks: vec![block(0, Vec::new())],
        next_value_id: 0,
        next_block_id: 0,
        next_slot_id: 0,
        debug_slots: HashMap::new(),
    }
}

fn declaration(id: usize, name: &str) -> TirFunction {
    function_with_blocks(id, name, Vec::new())
}

fn call(destination: usize, target: usize) -> TirInstr {
    TirInstr::Call(TirValueId(destination), TirFunctionId(target), Vec::new())
}

fn call_with_args(destination: usize, target: usize, arguments: Vec<TirValueId>) -> TirInstr {
    TirInstr::Call(TirValueId(destination), TirFunctionId(target), arguments)
}

fn string_const(id: usize, value: &str) -> TirInstr {
    TirInstr::Const(TirValueId(id), TirConst::String(value.to_owned()))
}

fn int_const(id: usize, value: i64) -> TirInstr {
    TirInstr::Const(TirValueId(id), TirConst::Int64(value))
}

fn specialized(fact: &LocalEffectFact) -> &SpecializedOperationFact {
    match fact {
        LocalEffectFact::Specialized(fact) => fact,
        LocalEffectFact::UnclassifiedOperation(fact) => {
            panic!("expected specialized fact, got unclassified {fact:?}")
        }
    }
}

fn synthetic_specialized(operation: EffectOperation) -> SpecializedOperationFact {
    SpecializedOperationFact {
        operation,
        resources: Vec::new(),
        location: TirLocation {
            function: TirFunctionId(0),
            block: TirBlockId(0),
            instruction_index: 0,
        },
        extern_id: TirFunctionId(100),
        extern_name: format!("synthetic.{}", operation.canonical_name()),
    }
}

fn resource_knowledge(
    fact: &SpecializedOperationFact,
    parameter: EffectParameter,
) -> &ResourceKnowledge {
    fact.resources
        .iter()
        .find(|resource| resource.parameter == parameter)
        .map(|resource| &resource.knowledge)
        .unwrap_or_else(|| {
            panic!(
                "{} does not declare {}",
                fact.operation.canonical_name(),
                parameter.canonical_name()
            )
        })
}

fn resource_parameters(fact: &SpecializedOperationFact) -> Vec<EffectParameter> {
    fact.resources
        .iter()
        .map(|resource| resource.parameter)
        .collect()
}

fn module(functions: Vec<TirFunction>) -> TirModule {
    TirModule { functions }
}

fn capabilities(items: &[Capability]) -> CapabilitySet {
    let mut result = CapabilitySet::empty();
    for capability in items {
        result.insert(*capability);
    }
    result
}

#[test]
fn capability_set_has_stable_order_and_set_operations() {
    let expected = [
        Capability::Alloc,
        Capability::Panic,
        Capability::Io,
        Capability::Process,
        Capability::Spawn,
        Capability::Nondeterminism,
        Capability::Unsafe,
    ];
    let mut set = CapabilitySet::empty();

    for capability in expected.into_iter().rev() {
        assert!(set.insert(capability));
        assert!(!set.insert(capability));
        assert!(set.contains(capability));
        assert_eq!(
            CapabilitySet::singleton(capability)
                .iter()
                .collect::<Vec<_>>(),
            vec![capability]
        );
    }

    assert_eq!(set.iter().collect::<Vec<_>>(), expected);
    assert_eq!(set.union(CapabilitySet::empty()), set);
    assert_eq!(CapabilitySet::default(), CapabilitySet::empty());
}

#[test]
fn capability_free_function_has_empty_direct_and_required_sets() {
    let analysis = analyze(&module(vec![function(7, "quiet", Vec::new())])).unwrap();
    let summary = analysis.summary(TirFunctionId(7)).unwrap();

    assert_eq!(summary.direct, CapabilitySet::empty());
    assert_eq!(summary.required, CapabilitySet::empty());
    assert!(analysis.direct_origins().is_empty());
    assert!(analysis.call_edges().is_empty());
}

#[test]
fn discovers_every_initial_tir_capability_mapping() {
    let instructions = vec![
        TirInstr::Const(TirValueId(0), TirConst::String("tempra".to_owned())),
        TirInstr::Make(TirValueId(1), TirType::Tuple(Vec::new()), None, Vec::new()),
        TirInstr::MakeStruct(TirValueId(2), "Point".to_owned(), Vec::new()),
        // E1 deliberately treats every division as may-panic. Proving a non-zero
        // divisor or refining IEEE floating-point behavior belongs to Phase V.
        TirInstr::Div(TirValueId(3), TirValueId(20), TirValueId(21)),
        TirInstr::Unwrap(TirValueId(4), TirValueId(22), "Some".to_owned(), 0),
        TirInstr::Get(TirValueId(5), TirValueId(23), TirKey::Index(0)),
        TirInstr::GetIndex(TirValueId(6), TirValueId(23), TirValueId(24)),
        TirInstr::Slice(
            TirValueId(7),
            TirValueId(23),
            TirValueId(24),
            TirValueId(25),
        ),
        TirInstr::Bytes(TirValueId(8), TirValueId(26)),
        TirInstr::ArrayPush(TirValueId(9), TirValueId(23), TirValueId(20)),
    ];
    let analysis = analyze(&module(vec![function(0, "all_direct", instructions)])).unwrap();
    let origins = analysis.direct_origins();

    assert_eq!(origins.len(), 10);
    let expected = [
        (Capability::Alloc, DirectCapabilitySource::StringConstant),
        (Capability::Alloc, DirectCapabilitySource::Make),
        (Capability::Alloc, DirectCapabilitySource::MakeStruct),
        (Capability::Panic, DirectCapabilitySource::Division),
        (Capability::Panic, DirectCapabilitySource::Unwrap),
        (Capability::Panic, DirectCapabilitySource::ConstantIndexGet),
        (Capability::Panic, DirectCapabilitySource::DynamicIndexGet),
        (Capability::Panic, DirectCapabilitySource::Slice),
        (Capability::Alloc, DirectCapabilitySource::Bytes),
        (Capability::Alloc, DirectCapabilitySource::ArrayPush),
    ];
    for (index, (capability, source)) in expected.into_iter().enumerate() {
        assert_eq!(origins[index].capability, capability);
        assert_eq!(origins[index].source, source);
        assert_eq!(origins[index].location.instruction_index, index);
    }

    let summary = analysis.summary(TirFunctionId(0)).unwrap();
    assert_eq!(
        summary.direct,
        capabilities(&[Capability::Alloc, Capability::Panic])
    );
    assert_eq!(summary.required, summary.direct);
}

#[test]
fn signed_and_unsigned_add_sub_mul_are_panic_origins_without_widening_float64() {
    let instructions = vec![
        TirInstr::Const(TirValueId(0), TirConst::Int64(1)),
        TirInstr::Const(TirValueId(1), TirConst::Int64(2)),
        TirInstr::Add(TirValueId(2), TirValueId(0), TirValueId(1)),
        TirInstr::Sub(TirValueId(3), TirValueId(0), TirValueId(1)),
        TirInstr::Mul(TirValueId(4), TirValueId(0), TirValueId(1)),
        TirInstr::Const(TirValueId(5), TirConst::UInt64(1)),
        TirInstr::Const(TirValueId(6), TirConst::UInt64(2)),
        TirInstr::Add(TirValueId(7), TirValueId(5), TirValueId(6)),
        TirInstr::Sub(TirValueId(8), TirValueId(5), TirValueId(6)),
        TirInstr::Mul(TirValueId(9), TirValueId(5), TirValueId(6)),
        TirInstr::Const(TirValueId(10), TirConst::Float64(1.0)),
        TirInstr::Const(TirValueId(11), TirConst::Float64(2.0)),
        TirInstr::Add(TirValueId(12), TirValueId(10), TirValueId(11)),
        TirInstr::Sub(TirValueId(13), TirValueId(10), TirValueId(11)),
        TirInstr::Mul(TirValueId(14), TirValueId(10), TirValueId(11)),
    ];
    let analysis = analyze(&module(vec![function(0, "typed_arithmetic", instructions)])).unwrap();

    let origins: Vec<_> = analysis
        .direct_origins()
        .iter()
        .map(|origin| (&origin.source, origin.location.instruction_index))
        .collect();
    assert_eq!(
        origins,
        vec![
            (&DirectCapabilitySource::IntegerAddition, 2),
            (&DirectCapabilitySource::IntegerSubtraction, 3),
            (&DirectCapabilitySource::IntegerMultiplication, 4),
            (&DirectCapabilitySource::IntegerAddition, 7),
            (&DirectCapabilitySource::IntegerSubtraction, 8),
            (&DirectCapabilitySource::IntegerMultiplication, 9),
        ]
    );
    assert_eq!(
        analysis.summary(TirFunctionId(0)).unwrap().direct,
        CapabilitySet::singleton(Capability::Panic)
    );
}

#[test]
fn classifies_all_known_externs_and_preserves_known_empty_sets() {
    let externs = [
        (100, "std.fs.exists", &[Capability::Io][..]),
        (
            101,
            "std.fs.read_text",
            &[Capability::Alloc, Capability::Io][..],
        ),
        (
            102,
            "std.fs.write_text",
            &[Capability::Alloc, Capability::Io][..],
        ),
        (103, "std.io.print", &[Capability::Io][..]),
        (104, "std.io.println", &[Capability::Io][..]),
        (105, "std.path.join", &[Capability::Alloc][..]),
        (106, "std.path.basename", &[Capability::Alloc][..]),
        (107, "std.strings.trim", &[Capability::Alloc][..]),
        (108, "std.strings.replace", &[Capability::Alloc][..]),
        (109, "std.strings.contains", &[][..]),
        (110, "std.strings.starts_with", &[][..]),
        (111, "std.strings.ends_with", &[][..]),
    ];
    let calls = externs
        .iter()
        .enumerate()
        .map(|(index, (id, _, _))| call(index, *id))
        .collect();
    let mut functions = vec![function(0, "caller", calls)];
    functions.extend(
        externs
            .iter()
            .map(|(id, name, _)| extern_function(*id, name)),
    );

    let analysis = analyze(&module(functions)).unwrap();
    let summary = analysis.summary(TirFunctionId(0)).unwrap();
    assert_eq!(
        summary.direct,
        capabilities(&[Capability::Alloc, Capability::Io])
    );
    assert_eq!(summary.required, summary.direct);
    assert_eq!(analysis.call_edges().len(), externs.len());
    assert_eq!(analysis.direct_origins().len(), 11);
    assert!(analysis.summary(TirFunctionId(100)).is_none());
    assert_eq!(
        analysis
            .origins_for_capability(TirFunctionId(0), Capability::Io)
            .count(),
        5
    );

    for (instruction_index, (id, name, expected)) in externs.iter().enumerate() {
        let actual = analysis
            .direct_origins()
            .iter()
            .filter(|origin| origin.location.instruction_index == instruction_index)
            .map(|origin| origin.capability)
            .collect::<Vec<_>>();
        assert_eq!(&actual, expected, "extern {name}");
        for origin in analysis
            .direct_origins()
            .iter()
            .filter(|origin| origin.location.instruction_index == instruction_index)
        {
            assert_eq!(
                origin.source,
                DirectCapabilitySource::ExternCall {
                    extern_id: TirFunctionId(*id),
                    name: (*name).to_owned(),
                }
            );
        }
    }
}

#[test]
fn unknown_unused_extern_is_accepted_but_called_extern_is_rejected() {
    let unused = module(vec![
        function(0, "main", Vec::new()),
        extern_function(1, "vendor.unknown"),
    ]);
    assert!(analyze(&unused).is_ok());

    let called = module(vec![
        function(0, "main", vec![call(0, 1)]),
        extern_function(1, "vendor.unknown"),
    ]);
    assert_eq!(
        analyze(&called),
        Err(CapabilityAnalysisError::UnclassifiedExtern {
            caller: TirFunctionId(0),
            extern_id: TirFunctionId(1),
            name: "vendor.unknown".to_owned(),
            callsite: crate::TirLocation {
                function: TirFunctionId(0),
                block: TirBlockId(0),
                instruction_index: 0,
            },
        })
    );

    let called_only_from_non_entry = module(vec![
        function(0, "main", Vec::new()),
        function(7, "helper", vec![call(0, 1)]),
        extern_function(1, "vendor.unknown"),
    ]);
    assert!(matches!(
        analyze(&called_only_from_non_entry),
        Err(CapabilityAnalysisError::UnclassifiedExtern {
            caller: TirFunctionId(7),
            extern_id: TirFunctionId(1),
            ..
        })
    ));
}

#[test]
fn analyzes_non_entry_functions() {
    let analysis = analyze(&module(vec![
        function(0, "main", Vec::new()),
        function(
            42,
            "unreachable_helper",
            vec![TirInstr::Const(
                TirValueId(0),
                TirConst::String("still analyzed".to_owned()),
            )],
        ),
    ]))
    .unwrap();

    assert_eq!(analysis.summaries().len(), 2);
    assert!(analysis
        .summary(TirFunctionId(42))
        .unwrap()
        .required
        .contains(Capability::Alloc));
}

#[test]
fn reports_malformed_function_call_and_declaration_ids_structurally() {
    let duplicate_function = module(vec![
        function(4, "one", Vec::new()),
        function(4, "two", Vec::new()),
    ]);
    assert_eq!(
        analyze(&duplicate_function),
        Err(CapabilityAnalysisError::DuplicateFunctionId {
            function: TirFunctionId(4),
        })
    );

    let unknown_target = module(vec![function(3, "caller", vec![call(0, 99)])]);
    assert!(matches!(
        analyze(&unknown_target),
        Err(CapabilityAnalysisError::UnknownCallTarget {
            caller: TirFunctionId(3),
            target: TirFunctionId(99),
            ..
        })
    ));

    let unresolved = module(vec![
        function(3, "caller", vec![call(0, 9)]),
        declaration(9, "declared_only"),
    ]);
    assert!(matches!(
        analyze(&unresolved),
        Err(CapabilityAnalysisError::UnresolvedDeclaration {
            caller: TirFunctionId(3),
            target: TirFunctionId(9),
            ref name,
            ..
        }) if name == "declared_only"
    ));
}

#[test]
fn duplicate_block_ids_are_function_scoped() {
    let duplicate = module(vec![function_with_blocks(
        5,
        "bad",
        vec![block(0, Vec::new()), block(0, Vec::new())],
    )]);
    assert_eq!(
        analyze(&duplicate),
        Err(CapabilityAnalysisError::DuplicateBlockId {
            function: TirFunctionId(5),
            block: TirBlockId(0),
        })
    );

    let shared_block_number = module(vec![
        function(5, "first", Vec::new()),
        function(8, "second", Vec::new()),
    ]);
    assert!(analyze(&shared_block_number).is_ok());
}

#[test]
fn propagates_capabilities_transitively_and_keeps_direct_separate() {
    let analysis = analyze(&module(vec![
        function(0, "a", vec![call(0, 1)]),
        function(1, "b", vec![call(0, 2), call(1, 3)]),
        function(
            2,
            "c",
            vec![TirInstr::Const(
                TirValueId(0),
                TirConst::String("alloc".to_owned()),
            )],
        ),
        function(
            3,
            "d",
            vec![TirInstr::Div(TirValueId(0), TirValueId(1), TirValueId(2))],
        ),
    ]))
    .unwrap();

    let both = capabilities(&[Capability::Alloc, Capability::Panic]);
    assert_eq!(
        analysis.summary(TirFunctionId(0)).unwrap().direct,
        CapabilitySet::empty()
    );
    assert_eq!(analysis.summary(TirFunctionId(0)).unwrap().required, both);
    assert_eq!(
        analysis.summary(TirFunctionId(1)).unwrap().direct,
        CapabilitySet::empty()
    );
    assert_eq!(analysis.summary(TirFunctionId(1)).unwrap().required, both);
    assert_eq!(
        analysis.summary(TirFunctionId(2)).unwrap().required,
        CapabilitySet::singleton(Capability::Alloc)
    );
    assert_eq!(
        analysis.summary(TirFunctionId(3)).unwrap().required,
        CapabilitySet::singleton(Capability::Panic)
    );
}

#[test]
fn fixed_point_converges_for_self_recursion_and_mutual_sccs() {
    let analysis = analyze(&module(vec![
        function(0, "self_recursive", vec![call(0, 0)]),
        function(1, "left", vec![call(0, 2)]),
        function(
            2,
            "right",
            vec![
                call(0, 1),
                TirInstr::Const(TirValueId(1), TirConst::String("alloc".to_owned())),
            ],
        ),
    ]))
    .unwrap();

    assert_eq!(
        analysis.summary(TirFunctionId(0)).unwrap().required,
        CapabilitySet::empty()
    );
    for id in [1, 2] {
        assert_eq!(
            analysis.summary(TirFunctionId(id)).unwrap().required,
            CapabilitySet::singleton(Capability::Alloc)
        );
    }
}

#[test]
fn witness_prefers_direct_then_shortest_then_function_id_tie_break() {
    let direct = analyze(&module(vec![
        function(
            0,
            "root",
            vec![
                call(0, 1),
                TirInstr::Const(TirValueId(1), TirConst::String("direct".to_owned())),
            ],
        ),
        function(
            1,
            "leaf",
            vec![TirInstr::Const(
                TirValueId(0),
                TirConst::String("leaf".to_owned()),
            )],
        ),
    ]))
    .unwrap();
    let direct_witness = direct.witness(TirFunctionId(0), Capability::Alloc).unwrap();
    assert!(direct_witness.calls.is_empty());
    assert_eq!(direct_witness.origin.location.function, TirFunctionId(0));

    let shortest = analyze(&module(vec![
        function(0, "root", vec![call(0, 1), call(1, 2)]),
        function(1, "middle", vec![call(0, 3)]),
        function(
            2,
            "short_leaf",
            vec![TirInstr::Const(
                TirValueId(0),
                TirConst::String("short".to_owned()),
            )],
        ),
        function(
            3,
            "long_leaf",
            vec![TirInstr::Const(
                TirValueId(0),
                TirConst::String("long".to_owned()),
            )],
        ),
    ]))
    .unwrap();
    let shortest_witness = shortest
        .witness(TirFunctionId(0), Capability::Alloc)
        .unwrap();
    assert_eq!(shortest_witness.calls.len(), 1);
    assert_eq!(shortest_witness.calls[0].callee, TirFunctionId(2));

    let tied = analyze(&module(vec![
        // Function 20 appears first in source order, but function ID is the
        // canonical tie-break before block and instruction position.
        function(0, "root", vec![call(0, 20), call(1, 10)]),
        function(
            20,
            "higher_id",
            vec![TirInstr::Const(
                TirValueId(0),
                TirConst::String("higher".to_owned()),
            )],
        ),
        function(
            10,
            "lower_id",
            vec![TirInstr::Const(
                TirValueId(0),
                TirConst::String("lower".to_owned()),
            )],
        ),
    ]))
    .unwrap();
    let tied_witness = tied.witness(TirFunctionId(0), Capability::Alloc).unwrap();
    assert_eq!(tied_witness.calls[0].callee, TirFunctionId(10));
    assert_eq!(tied_witness.origin.location.function, TirFunctionId(10));
}

#[test]
fn witness_uses_block_and_instruction_tie_breaks() {
    let analysis = analyze(&module(vec![
        function_with_blocks(
            0,
            "root",
            // Deliberately store the numerically later block first.
            vec![
                block(9, vec![call(0, 1)]),
                block(3, vec![call(0, 1), call(1, 1)]),
            ],
        ),
        function_with_blocks(
            1,
            "leaf",
            vec![
                block(
                    8,
                    vec![TirInstr::Const(
                        TirValueId(0),
                        TirConst::String("later origin".to_owned()),
                    )],
                ),
                block(
                    2,
                    vec![
                        TirInstr::Const(TirValueId(1), TirConst::Int64(0)),
                        TirInstr::Const(
                            TirValueId(2),
                            TirConst::String("canonical origin".to_owned()),
                        ),
                    ],
                ),
            ],
        ),
    ]))
    .unwrap();

    let witness = analysis
        .witness(TirFunctionId(0), Capability::Alloc)
        .unwrap();
    assert_eq!(witness.calls.len(), 1);
    assert_eq!(witness.calls[0].callsite.block, TirBlockId(3));
    assert_eq!(witness.calls[0].callsite.instruction_index, 0);
    assert_eq!(witness.origin.location.block, TirBlockId(2));
    assert_eq!(witness.origin.location.instruction_index, 1);
}

#[test]
fn analysis_is_identical_across_repeated_runs() {
    let input = module(vec![
        function(9, "caller", vec![call(0, 2), call(1, 5)]),
        function(
            5,
            "panic",
            vec![TirInstr::Div(TirValueId(0), TirValueId(1), TirValueId(2))],
        ),
        function(
            2,
            "alloc",
            vec![TirInstr::Const(
                TirValueId(0),
                TirConst::String("stable".to_owned()),
            )],
        ),
    ]);

    let first = analyze(&input).unwrap();
    for _ in 0..32 {
        assert_eq!(analyze(&input).unwrap(), first);
    }
}

#[test]
fn effect_operations_have_stable_exhaustive_order() {
    let expected = [
        EffectOperation::FsRead,
        EffectOperation::FsWrite,
        EffectOperation::FsRemove,
        EffectOperation::NetConnect,
        EffectOperation::NetListen,
        EffectOperation::NetSend,
        EffectOperation::NetReceive,
        EffectOperation::IoRead,
        EffectOperation::IoWrite,
        EffectOperation::ProcessExec,
        EffectOperation::ProcessExit,
        EffectOperation::SpawnTask,
        EffectOperation::SpawnActor,
    ];

    // Comparing against the complete literal list keeps the closed taxonomy
    // honest: a new variant fails this test until it is explicitly registered.
    assert_eq!(EffectOperation::ALL, expected);
}

#[test]
fn effect_operation_discriminants_preserve_existing_identities() {
    assert_eq!(EffectOperation::FsRead as u8, 0);
    assert_eq!(EffectOperation::FsWrite as u8, 1);
    assert_eq!(EffectOperation::FsRemove as u8, 2);
    assert_eq!(EffectOperation::NetConnect as u8, 3);
    assert_eq!(EffectOperation::NetListen as u8, 4);
    assert_eq!(EffectOperation::IoRead as u8, 5);
    assert_eq!(EffectOperation::IoWrite as u8, 6);
    assert_eq!(EffectOperation::ProcessExec as u8, 7);
    assert_eq!(EffectOperation::ProcessExit as u8, 8);
    assert_eq!(EffectOperation::SpawnTask as u8, 9);
    assert_eq!(EffectOperation::SpawnActor as u8, 10);
    assert_eq!(EffectOperation::NetSend as u8, 11);
    assert_eq!(EffectOperation::NetReceive as u8, 12);
}

#[test]
fn effect_operation_names_are_unique_and_canonical() {
    let expected = [
        (EffectOperation::FsRead, "fs.read"),
        (EffectOperation::FsWrite, "fs.write"),
        (EffectOperation::FsRemove, "fs.remove"),
        (EffectOperation::NetConnect, "net.connect"),
        (EffectOperation::NetListen, "net.listen"),
        (EffectOperation::NetSend, "net.send"),
        (EffectOperation::NetReceive, "net.receive"),
        (EffectOperation::IoRead, "io.read"),
        (EffectOperation::IoWrite, "io.write"),
        (EffectOperation::ProcessExec, "process.exec"),
        (EffectOperation::ProcessExit, "process.exit"),
        (EffectOperation::SpawnTask, "spawn.task"),
        (EffectOperation::SpawnActor, "spawn.actor"),
    ];

    for (operation, name) in expected {
        assert_eq!(operation.canonical_name(), name);
    }

    let mut names: Vec<_> = EffectOperation::ALL
        .iter()
        .map(|operation| operation.canonical_name())
        .collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), EffectOperation::ALL.len());
}

#[test]
fn effect_operations_map_to_exactly_one_existing_parent_capability() {
    let expected = [
        (EffectOperation::FsRead, Capability::Io),
        (EffectOperation::FsWrite, Capability::Io),
        (EffectOperation::FsRemove, Capability::Io),
        (EffectOperation::NetConnect, Capability::Io),
        (EffectOperation::NetListen, Capability::Io),
        (EffectOperation::NetSend, Capability::Io),
        (EffectOperation::NetReceive, Capability::Io),
        (EffectOperation::IoRead, Capability::Io),
        (EffectOperation::IoWrite, Capability::Io),
        (EffectOperation::ProcessExec, Capability::Process),
        (EffectOperation::ProcessExit, Capability::Process),
        (EffectOperation::SpawnTask, Capability::Spawn),
        (EffectOperation::SpawnActor, Capability::Spawn),
    ];

    for (operation, parent) in expected {
        assert_eq!(operation.parent(), parent, "{}", operation.canonical_name());
    }

    for operation in EffectOperation::ALL {
        assert!(
            matches!(
                operation.parent(),
                Capability::Io | Capability::Process | Capability::Spawn
            ),
            "{} has a non-taxonomy parent",
            operation.canonical_name()
        );
    }
}

#[test]
fn effect_families_are_explicit_and_do_not_change_parents() {
    assert_eq!(EffectFamily::ALL, [EffectFamily::Fs, EffectFamily::Net]);
    assert_eq!(EffectFamily::from_name("fs"), Some(EffectFamily::Fs));
    assert_eq!(EffectFamily::from_name("net"), Some(EffectFamily::Net));
    assert_eq!(EffectFamily::from_name("io"), None);
    assert_eq!(EffectFamily::Fs.canonical_name(), "fs");
    assert_eq!(EffectFamily::Net.canonical_name(), "net");

    let expected = [
        (EffectOperation::FsRead, Some(EffectFamily::Fs)),
        (EffectOperation::FsWrite, Some(EffectFamily::Fs)),
        (EffectOperation::FsRemove, Some(EffectFamily::Fs)),
        (EffectOperation::NetConnect, Some(EffectFamily::Net)),
        (EffectOperation::NetListen, Some(EffectFamily::Net)),
        (EffectOperation::NetSend, Some(EffectFamily::Net)),
        (EffectOperation::NetReceive, Some(EffectFamily::Net)),
        (EffectOperation::IoRead, None),
        (EffectOperation::IoWrite, None),
        (EffectOperation::ProcessExec, None),
        (EffectOperation::ProcessExit, None),
        (EffectOperation::SpawnTask, None),
        (EffectOperation::SpawnActor, None),
    ];

    for (operation, family) in expected {
        assert_eq!(operation.family(), family, "{}", operation.canonical_name());
    }

    for operation in EffectOperation::ALL {
        if let Some(family) = operation.family() {
            assert_eq!(operation.parent(), Capability::Io);
            assert!(
                EffectFamily::ALL.contains(&family),
                "{} has an unknown family",
                operation.canonical_name()
            );
        }
    }
}

#[test]
fn cross_family_categories_have_exact_membership() {
    assert_eq!(
        EffectCategory::ALL,
        [EffectCategory::IoRead, EffectCategory::IoWrite]
    );
    assert_eq!(
        EffectCategory::from_name("io.read"),
        Some(EffectCategory::IoRead)
    );
    assert_eq!(
        EffectCategory::from_name("io.write"),
        Some(EffectCategory::IoWrite)
    );
    assert_eq!(EffectCategory::from_name("io"), None);
    assert_eq!(EffectCategory::IoRead.canonical_name(), "io.read");
    assert_eq!(EffectCategory::IoWrite.canonical_name(), "io.write");

    let expected = [
        (EffectOperation::FsRead, &[EffectCategory::IoRead][..]),
        (EffectOperation::FsWrite, &[EffectCategory::IoWrite][..]),
        (EffectOperation::FsRemove, &[][..]),
        (EffectOperation::NetConnect, &[][..]),
        (EffectOperation::NetListen, &[][..]),
        (EffectOperation::NetSend, &[EffectCategory::IoWrite][..]),
        (EffectOperation::NetReceive, &[EffectCategory::IoRead][..]),
        (EffectOperation::IoRead, &[EffectCategory::IoRead][..]),
        (EffectOperation::IoWrite, &[EffectCategory::IoWrite][..]),
        (EffectOperation::ProcessExec, &[][..]),
        (EffectOperation::ProcessExit, &[][..]),
        (EffectOperation::SpawnTask, &[][..]),
        (EffectOperation::SpawnActor, &[][..]),
    ];

    for (operation, categories) in expected {
        assert_eq!(
            operation.categories(),
            categories,
            "{}",
            operation.canonical_name()
        );
        for category in EffectCategory::ALL {
            assert_eq!(
                operation.is_in_category(category),
                categories.contains(&category),
                "{} / {}",
                operation.canonical_name(),
                category.canonical_name()
            );
        }
    }

    assert!(!EffectOperation::FsRemove.is_in_category(EffectCategory::IoWrite));
    assert!(!EffectOperation::NetConnect.is_in_category(EffectCategory::IoRead));
    assert!(!EffectOperation::NetConnect.is_in_category(EffectCategory::IoWrite));
    assert!(!EffectOperation::NetListen.is_in_category(EffectCategory::IoRead));
    assert!(!EffectOperation::NetListen.is_in_category(EffectCategory::IoWrite));
}

#[test]
fn fact_classification_accessors_agree_with_taxonomy_for_every_operation() {
    for operation in EffectOperation::ALL {
        let specialized = synthetic_specialized(operation);

        assert_eq!(specialized.capability(), operation.parent());
        assert_eq!(specialized.family(), operation.family());
        assert_eq!(specialized.categories(), operation.categories());
        for category in EffectCategory::ALL {
            assert_eq!(
                specialized.is_in_category(category),
                operation.is_in_category(category),
                "{} / {}",
                operation.canonical_name(),
                category.canonical_name()
            );
        }

        let fact = LocalEffectFact::Specialized(specialized);
        assert_eq!(fact.operation(), Some(operation));
        assert_eq!(fact.capability(), operation.parent());
        for family in EffectFamily::ALL {
            assert_eq!(
                fact.family_membership(family),
                Some(operation.family() == Some(family)),
                "{} / {}",
                operation.canonical_name(),
                family.canonical_name()
            );
        }
        for category in EffectCategory::ALL {
            assert_eq!(
                fact.category_membership(category),
                Some(operation.is_in_category(category)),
                "{} / {}",
                operation.canonical_name(),
                category.canonical_name()
            );
        }
    }
}

#[test]
fn unclassified_facts_keep_family_and_category_membership_unknown() {
    let fact = LocalEffectFact::UnclassifiedOperation(crate::UnclassifiedOperationFact {
        capability: Capability::Io,
        location: TirLocation {
            function: TirFunctionId(0),
            block: TirBlockId(0),
            instruction_index: 0,
        },
        extern_id: TirFunctionId(100),
        extern_name: "std.fs.exists".to_owned(),
    });

    assert_eq!(fact.operation(), None);
    assert_eq!(fact.capability(), Capability::Io);
    for family in EffectFamily::ALL {
        assert_eq!(fact.family_membership(family), None, "{family:?}");
    }
    for category in EffectCategory::ALL {
        assert_eq!(fact.category_membership(category), None, "{category:?}");
    }
}

#[test]
fn effect_parameters_are_valid_only_where_declared() {
    let expected: [(EffectOperation, &[EffectParameter]); 13] = [
        (EffectOperation::FsRead, &[EffectParameter::Source]),
        (EffectOperation::FsWrite, &[EffectParameter::Destination]),
        (EffectOperation::FsRemove, &[EffectParameter::Destination]),
        (
            EffectOperation::NetConnect,
            &[
                EffectParameter::Host,
                EffectParameter::Port,
                EffectParameter::Protocol,
            ],
        ),
        (
            EffectOperation::NetListen,
            &[
                EffectParameter::Host,
                EffectParameter::Port,
                EffectParameter::Protocol,
            ],
        ),
        (EffectOperation::NetSend, &[]),
        (EffectOperation::NetReceive, &[]),
        (EffectOperation::IoRead, &[EffectParameter::Stream]),
        (EffectOperation::IoWrite, &[EffectParameter::Stream]),
        (EffectOperation::ProcessExec, &[EffectParameter::Executable]),
        (EffectOperation::ProcessExit, &[]),
        (EffectOperation::SpawnTask, &[EffectParameter::Scope]),
        (EffectOperation::SpawnActor, &[EffectParameter::Scope]),
    ];

    for (operation, keys) in expected {
        assert_eq!(
            operation.valid_parameter_keys(),
            keys,
            "{}",
            operation.canonical_name()
        );
        for parameter in EffectParameter::ALL {
            assert_eq!(
                operation.accepts_parameter(parameter),
                keys.contains(&parameter),
                "{} / {}",
                operation.canonical_name(),
                parameter.canonical_name()
            );
        }
    }
}

#[test]
fn effect_parameter_vocabulary_is_closed_and_stable() {
    let expected = [
        EffectParameter::Source,
        EffectParameter::Destination,
        EffectParameter::Host,
        EffectParameter::Port,
        EffectParameter::Protocol,
        EffectParameter::Executable,
        EffectParameter::Stream,
        EffectParameter::Scope,
    ];
    assert_eq!(EffectParameter::ALL, expected);

    let expected_names = [
        (EffectParameter::Source, "source"),
        (EffectParameter::Destination, "destination"),
        (EffectParameter::Host, "host"),
        (EffectParameter::Port, "port"),
        (EffectParameter::Protocol, "protocol"),
        (EffectParameter::Executable, "executable"),
        (EffectParameter::Stream, "stream"),
        (EffectParameter::Scope, "scope"),
    ];
    for (parameter, name) in expected_names {
        assert_eq!(parameter.canonical_name(), name);
    }

    let mut names: Vec<_> = EffectParameter::ALL
        .iter()
        .map(|parameter| parameter.canonical_name())
        .collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), EffectParameter::ALL.len());
}

#[test]
fn specialized_registry_does_not_consume_capability_bits() {
    assert_eq!(std::mem::size_of::<CapabilitySet>(), 1);
    assert_eq!(std::mem::size_of::<Capability>(), 1);
    assert_eq!(
        [
            Capability::Alloc,
            Capability::Panic,
            Capability::Io,
            Capability::Process,
            Capability::Spawn,
            Capability::Nondeterminism,
            Capability::Unsafe,
        ]
        .map(|capability| capability as u8),
        [0, 1, 2, 3, 4, 5, 6]
    );

    let parents = EffectOperation::ALL
        .iter()
        .fold(CapabilitySet::empty(), |set, operation| {
            set.union(CapabilitySet::singleton(operation.parent()))
        });
    assert_eq!(
        parents.iter().collect::<Vec<_>>(),
        vec![Capability::Io, Capability::Process, Capability::Spawn]
    );
}

#[test]
fn deferred_registry_metadata_does_not_imply_runtime_support() {
    assert!(EffectOperation::ProcessExit
        .valid_parameter_keys()
        .is_empty());
    assert_eq!(
        EffectOperation::SpawnTask.valid_parameter_keys(),
        &[EffectParameter::Scope]
    );
    assert_eq!(
        EffectOperation::SpawnActor.valid_parameter_keys(),
        &[EffectParameter::Scope]
    );

    // Registering the vocabulary does not classify externs or make these
    // operations executable: the existing analysis still rejects both calls as
    // unclassified, and no runtime availability is claimed.
    for name in ["std.spawn.task", "std.process.exit"] {
        let called = module(vec![
            function(0, "main", vec![call(0, 1)]),
            extern_function(1, name),
        ]);
        assert!(
            matches!(
                analyze(&called),
                Err(CapabilityAnalysisError::UnclassifiedExtern { .. })
            ),
            "{name} must remain unclassified"
        );
    }
}

#[test]
fn specialized_operation_for_extern_mappings_are_exact() {
    let expected = [
        ("std.fs.read_text", Some(EffectOperation::FsRead)),
        ("std.fs.write_text", Some(EffectOperation::FsWrite)),
        ("std.io.print", Some(EffectOperation::IoWrite)),
        ("std.io.println", Some(EffectOperation::IoWrite)),
    ];

    for (name, operation) in expected {
        assert_eq!(operation_for_extern(name), operation, "{name}");
    }
}

#[test]
fn std_fs_exists_remains_unclassified_without_forcing_fs_read() {
    // The extern stays known for broad capability analysis...
    assert_eq!(
        capabilities_for_extern("std.fs.exists"),
        Some(CapabilitySet::singleton(Capability::Io))
    );
    // ...but deliberately has no specialized operation.
    assert_eq!(operation_for_extern("std.fs.exists"), None);
}

#[test]
fn alloc_only_and_capability_free_externs_have_no_specialized_operation() {
    for name in [
        "std.path.join",
        "std.path.basename",
        "std.strings.trim",
        "std.strings.replace",
        "std.strings.contains",
        "std.strings.starts_with",
        "std.strings.ends_with",
    ] {
        assert!(
            capabilities_for_extern(name).is_some(),
            "{name} must stay known"
        );
        assert_eq!(operation_for_extern(name), None, "{name}");
    }
}

#[test]
fn unknown_externs_have_no_operation_and_stay_unclassified_errors() {
    for name in ["vendor.unknown", "", "std.fs.read", "std.io.readln"] {
        assert_eq!(capabilities_for_extern(name), None, "{name}");
        assert_eq!(operation_for_extern(name), None, "{name}");

        let called = module(vec![
            function(0, "main", vec![call(0, 1)]),
            extern_function(1, name),
        ]);
        assert!(
            matches!(
                analyze(&called),
                Err(CapabilityAnalysisError::UnclassifiedExtern { .. })
            ),
            "{name} must remain unclassified"
        );
    }
}

#[test]
fn every_classified_operation_parent_is_in_its_extern_capability_set() {
    let classified = [
        ("std.fs.read_text", EffectOperation::FsRead),
        ("std.fs.write_text", EffectOperation::FsWrite),
        ("std.io.print", EffectOperation::IoWrite),
        ("std.io.println", EffectOperation::IoWrite),
    ];

    for (name, operation) in classified {
        let capabilities =
            capabilities_for_extern(name).unwrap_or_else(|| panic!("{name} must stay known"));
        assert!(
            capabilities.contains(operation.parent()),
            "{name}: parent {:?} is missing from its capability set",
            operation.parent()
        );
        assert_eq!(operation_for_extern(name), Some(operation), "{name}");
    }
}

#[test]
fn specialized_classification_does_not_change_broad_capability_requirements() {
    let expected = [
        ("std.fs.exists", &[Capability::Io][..]),
        ("std.fs.read_text", &[Capability::Alloc, Capability::Io][..]),
        (
            "std.fs.write_text",
            &[Capability::Alloc, Capability::Io][..],
        ),
        ("std.io.print", &[Capability::Io][..]),
        ("std.io.println", &[Capability::Io][..]),
        ("std.path.join", &[Capability::Alloc][..]),
        ("std.path.basename", &[Capability::Alloc][..]),
        ("std.strings.trim", &[Capability::Alloc][..]),
        ("std.strings.replace", &[Capability::Alloc][..]),
        ("std.strings.contains", &[][..]),
        ("std.strings.starts_with", &[][..]),
        ("std.strings.ends_with", &[][..]),
    ];

    for (name, capabilities_expected) in expected {
        assert_eq!(
            capabilities_for_extern(name),
            Some(capabilities(capabilities_expected)),
            "{name}"
        );
    }
}

#[test]
fn extracts_exact_filesystem_literals_and_ignores_write_content() {
    let caller = function(
        0,
        "main",
        vec![
            string_const(0, "input.txt"),
            call_with_args(1, 100, vec![TirValueId(0)]),
            string_const(2, "build/out.txt"),
            string_const(3, "file contents"),
            call_with_args(4, 101, vec![TirValueId(2), TirValueId(3)]),
        ],
    );
    let analysis = analyze(&module(vec![
        caller,
        extern_function(100, "std.fs.read_text"),
        extern_function(101, "std.fs.write_text"),
    ]))
    .unwrap();

    let facts = analysis.local_effect_facts();
    assert_eq!(facts.len(), 2);

    let read = specialized(&facts[0]);
    assert_eq!(read.operation, EffectOperation::FsRead);
    assert_eq!(read.resources.len(), 1);
    assert_eq!(resource_parameters(read), vec![EffectParameter::Source]);
    assert_eq!(
        *resource_knowledge(read, EffectParameter::Source),
        ResourceKnowledge::Exact(ResourceValue::Text("input.txt".to_owned()))
    );
    assert_eq!(
        read.location,
        TirLocation {
            function: TirFunctionId(0),
            block: TirBlockId(0),
            instruction_index: 1,
        }
    );
    assert_eq!(read.extern_id, TirFunctionId(100));
    assert_eq!(read.extern_name, "std.fs.read_text");

    let write = specialized(&facts[1]);
    assert_eq!(write.operation, EffectOperation::FsWrite);
    assert_eq!(
        resource_parameters(write),
        vec![EffectParameter::Destination]
    );
    assert_eq!(
        *resource_knowledge(write, EffectParameter::Destination),
        ResourceKnowledge::Exact(ResourceValue::Text("build/out.txt".to_owned()))
    );
    // The second write_text argument is file content, not the destination.
    assert_ne!(
        *resource_knowledge(write, EffectParameter::Destination),
        ResourceKnowledge::Exact(ResourceValue::Text("file contents".to_owned()))
    );
    assert_eq!(write.location.instruction_index, 4);
}

#[test]
fn resource_value_kind_must_match_the_bound_parameter() {
    // Verified TIR cannot pass an integer path, but a malformed or future TIR
    // producer must not turn a numeric constant into an exact text resource.
    let caller = function(
        0,
        "main",
        vec![
            int_const(0, 8080),
            call_with_args(1, 100, vec![TirValueId(0)]),
        ],
    );
    let analysis = analyze(&module(vec![
        caller,
        extern_function(100, "std.fs.read_text"),
    ]))
    .unwrap();

    let fact = specialized(&analysis.local_effect_facts()[0]);
    assert_eq!(fact.operation, EffectOperation::FsRead);
    assert_eq!(
        *resource_knowledge(fact, EffectParameter::Source),
        ResourceKnowledge::Unknown
    );
}

#[test]
fn dynamic_filesystem_arguments_keep_identity_with_unknown_resource() {
    // A slot load and a parameter are both deliberately opaque.
    let reader = function(
        0,
        "reader",
        vec![
            TirInstr::Slot(TirSlotId(0), TirType::String),
            string_const(0, "dynamic.txt"),
            TirInstr::Store(TirSlotId(0), TirValueId(0)),
            TirInstr::Load(TirValueId(1), TirSlotId(0)),
            call_with_args(2, 100, vec![TirValueId(1)]),
        ],
    );
    let mut writer = function(
        1,
        "writer",
        vec![call_with_args(9, 101, vec![TirValueId(0)])],
    );
    writer.params = vec![(TirValueId(0), TirType::String)];

    let analysis = analyze(&module(vec![
        reader,
        writer,
        extern_function(100, "std.fs.read_text"),
        extern_function(101, "std.fs.write_text"),
    ]))
    .unwrap();

    let facts = analysis.local_effect_facts();
    assert_eq!(facts.len(), 2);

    let read = specialized(&facts[0]);
    assert_eq!(read.operation, EffectOperation::FsRead);
    assert_eq!(
        *resource_knowledge(read, EffectParameter::Source),
        ResourceKnowledge::Unknown
    );
    assert_eq!(read.location.function, TirFunctionId(0));

    let write = specialized(&facts[1]);
    assert_eq!(write.operation, EffectOperation::FsWrite);
    assert_eq!(
        *resource_knowledge(write, EffectParameter::Destination),
        ResourceKnowledge::Unknown
    );
    assert_eq!(write.location.function, TirFunctionId(1));
}

#[test]
fn known_capability_without_operation_is_explicitly_unclassified() {
    let caller = function(
        0,
        "main",
        vec![
            string_const(0, "."),
            call_with_args(1, 100, vec![TirValueId(0)]),
            call(2, 101),
        ],
    );

    let analysis = analyze(&module(vec![
        caller,
        extern_function(100, "std.fs.exists"),
        extern_function(101, "std.path.join"),
    ]))
    .unwrap();

    let facts = analysis.local_effect_facts();
    // `std.fs.exists` is Io but has no specialized operation. `std.path.join` is
    // Alloc-only, and alloc refinements are deferred, so it produces no fact.
    assert_eq!(facts.len(), 1);
    match &facts[0] {
        LocalEffectFact::UnclassifiedOperation(fact) => {
            assert_eq!(fact.capability, Capability::Io);
            assert_eq!(fact.extern_name, "std.fs.exists");
            assert_eq!(fact.extern_id, TirFunctionId(100));
            assert_eq!(
                fact.location,
                TirLocation {
                    function: TirFunctionId(0),
                    block: TirBlockId(0),
                    instruction_index: 1,
                }
            );
        }
        other => panic!("expected unclassified operation, got {other:?}"),
    }
}

#[test]
fn print_and_println_never_treat_content_as_the_stream() {
    let caller = function(
        0,
        "main",
        vec![
            string_const(0, "hello"),
            call_with_args(1, 100, vec![TirValueId(0)]),
            string_const(2, "world"),
            call_with_args(3, 101, vec![TirValueId(2)]),
        ],
    );
    let analysis = analyze(&module(vec![
        caller,
        extern_function(100, "std.io.print"),
        extern_function(101, "std.io.println"),
    ]))
    .unwrap();

    let facts = analysis.local_effect_facts();
    assert_eq!(facts.len(), 2);
    for (fact, content) in facts.iter().zip(["hello", "world"]) {
        let fact = specialized(fact);
        assert_eq!(fact.operation, EffectOperation::IoWrite);
        assert_eq!(resource_parameters(fact), vec![EffectParameter::Stream]);
        assert_eq!(
            *resource_knowledge(fact, EffectParameter::Stream),
            ResourceKnowledge::Unknown
        );
        assert_ne!(
            *resource_knowledge(fact, EffectParameter::Stream),
            ResourceKnowledge::Exact(ResourceValue::Text(content.to_owned()))
        );
    }
}

#[test]
fn multiple_calls_have_stable_ordered_locations() {
    let early = function(
        3,
        "early",
        vec![
            string_const(0, "early.txt"),
            call_with_args(1, 100, vec![TirValueId(0)]),
        ],
    );
    let late = function_with_blocks(
        9,
        "late",
        vec![
            // Deliberately store the numerically later block first.
            block(
                7,
                vec![
                    string_const(1, "seven.txt"),
                    call_with_args(2, 100, vec![TirValueId(1)]),
                ],
            ),
            block(
                2,
                vec![
                    string_const(0, "two.txt"),
                    call_with_args(1, 100, vec![TirValueId(0)]),
                ],
            ),
        ],
    );
    let input = module(vec![late, early, extern_function(100, "std.fs.read_text")]);
    let analysis = analyze(&input).unwrap();
    let facts = analysis.local_effect_facts();

    let locations: Vec<_> = facts.iter().map(LocalEffectFact::location).collect();
    assert_eq!(
        locations,
        vec![
            TirLocation {
                function: TirFunctionId(3),
                block: TirBlockId(0),
                instruction_index: 1,
            },
            TirLocation {
                function: TirFunctionId(9),
                block: TirBlockId(2),
                instruction_index: 1,
            },
            TirLocation {
                function: TirFunctionId(9),
                block: TirBlockId(7),
                instruction_index: 1,
            },
        ]
    );

    assert_eq!(analyze(&input).unwrap(), analysis);
}

#[test]
fn non_dominating_ambiguous_and_indirect_definitions_are_unknown() {
    let cross_block = function_with_blocks(
        0,
        "cross_block",
        vec![
            block_with_terminator(
                0,
                vec![TirInstr::Const(TirValueId(9), TirConst::Bool(true))],
                TirTerminator::CondBr(TirValueId(9), TirBlockId(1), TirBlockId(2)),
            ),
            // The constant is defined in one sibling branch...
            block_with_terminator(
                1,
                vec![string_const(0, "cross")],
                TirTerminator::Br(TirBlockId(3)),
            ),
            // ...but the call is in the other, so the definition does not
            // dominate the callsite.
            block_with_terminator(
                2,
                vec![call_with_args(1, 100, vec![TirValueId(0)])],
                TirTerminator::Br(TirBlockId(3)),
            ),
            block(3, Vec::new()),
        ],
    );
    let after_call = function(
        1,
        "after_call",
        vec![
            call_with_args(1, 100, vec![TirValueId(0)]),
            string_const(0, "too late"),
        ],
    );
    let ambiguous = function(
        2,
        "ambiguous",
        vec![
            string_const(0, "first"),
            string_const(0, "second"),
            call_with_args(1, 100, vec![TirValueId(0)]),
        ],
    );
    let mut parameter = function(
        3,
        "parameter",
        vec![call_with_args(1, 100, vec![TirValueId(0)])],
    );
    parameter.params = vec![(TirValueId(0), TirType::String)];

    let analysis = analyze(&module(vec![
        cross_block,
        after_call,
        ambiguous,
        parameter,
        extern_function(100, "std.fs.read_text"),
    ]))
    .unwrap();

    let facts = analysis.local_effect_facts();
    assert_eq!(facts.len(), 4);
    for fact in facts {
        let fact = specialized(fact);
        assert_eq!(fact.operation, EffectOperation::FsRead);
        assert_eq!(
            *resource_knowledge(fact, EffectParameter::Source),
            ResourceKnowledge::Unknown
        );
    }
}

#[test]
fn local_fact_extraction_does_not_change_broad_capability_requirements() {
    let caller = function(
        0,
        "main",
        vec![
            string_const(0, "input.txt"),
            call_with_args(1, 100, vec![TirValueId(0)]),
            string_const(2, "build/out.txt"),
            string_const(3, "file contents"),
            call_with_args(4, 101, vec![TirValueId(2), TirValueId(3)]),
        ],
    );
    let analysis = analyze(&module(vec![
        caller,
        extern_function(100, "std.fs.read_text"),
        extern_function(101, "std.fs.write_text"),
    ]))
    .unwrap();

    let summary = analysis.summary(TirFunctionId(0)).unwrap();
    assert_eq!(
        summary.direct,
        capabilities(&[Capability::Alloc, Capability::Io])
    );
    assert_eq!(summary.required, summary.direct);
    assert_eq!(
        analysis
            .origins_for_capability(TirFunctionId(0), Capability::Io)
            .count(),
        2
    );
    // Three string constants plus the Alloc half of both extern calls.
    assert_eq!(
        analysis
            .origins_for_capability(TirFunctionId(0), Capability::Alloc)
            .count(),
        5
    );
}

#[test]
fn numeric_constants_have_exact_port_resource_values() {
    let body = function(
        0,
        "port",
        vec![
            int_const(0, 8080),
            call_with_args(1, 100, vec![TirValueId(0)]),
        ],
    );
    let index = crate::facts::FunctionValueIndex::new(&body);
    let callsite = TirLocation {
        function: TirFunctionId(0),
        block: TirBlockId(0),
        instruction_index: 1,
    };
    assert_eq!(
        index.resolve(TirValueId(0), callsite),
        ResourceKnowledge::Exact(ResourceValue::Port(8080))
    );

    for value in [-1, 65_536] {
        let body = function(
            0,
            "port",
            vec![
                int_const(0, value),
                call_with_args(1, 100, vec![TirValueId(0)]),
            ],
        );
        let index = crate::facts::FunctionValueIndex::new(&body);
        assert_eq!(
            index.resolve(TirValueId(0), callsite),
            ResourceKnowledge::Unknown,
            "{value}"
        );
    }
}

#[test]
fn unknown_externs_never_become_local_facts() {
    let unused = module(vec![
        function(0, "main", Vec::new()),
        extern_function(1, "vendor.unknown"),
    ]);
    assert!(analyze(&unused).unwrap().local_effect_facts().is_empty());

    let called = module(vec![
        function(0, "main", vec![call(0, 1)]),
        extern_function(1, "vendor.unknown"),
    ]);
    assert!(matches!(
        analyze(&called),
        Err(CapabilityAnalysisError::UnclassifiedExtern { .. })
    ));
}

#[test]
fn native_operation_bindings_are_valid_and_canonical() {
    let expected = [
        (
            "std.fs.read_text",
            EffectOperation::FsRead,
            EffectParameter::Source,
            Some(0),
        ),
        (
            "std.fs.write_text",
            EffectOperation::FsWrite,
            EffectParameter::Destination,
            Some(0),
        ),
        (
            "std.io.print",
            EffectOperation::IoWrite,
            EffectParameter::Stream,
            None,
        ),
        (
            "std.io.println",
            EffectOperation::IoWrite,
            EffectParameter::Stream,
            None,
        ),
    ];

    for (name, operation, parameter, argument_index) in expected {
        let binding = operation_binding_for_extern(name).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(binding.operation, operation, "{name}");
        assert_eq!(
            binding.resources,
            vec![ResourceBinding {
                parameter,
                argument_index,
            }],
            "{name}"
        );
        // Re-validating the table entry proves the static metadata satisfies the
        // same invariants as synthetic bindings.
        assert!(
            ExternOperationBinding::new(binding.operation, binding.resources).is_ok(),
            "{name}"
        );
    }
}

#[test]
fn synthetic_multi_parameter_binding_orders_and_extracts_resources() {
    // Declared deliberately out of order. This binding is internal test metadata
    // only; no net extern or native operation is added.
    let binding = ExternOperationBinding::new(
        EffectOperation::NetConnect,
        [
            ResourceBinding::unknown(EffectParameter::Protocol),
            ResourceBinding::argument(EffectParameter::Port, 1),
            ResourceBinding::argument(EffectParameter::Host, 0),
        ],
    )
    .unwrap();
    assert_eq!(
        binding
            .resources
            .iter()
            .map(|resource| resource.parameter)
            .collect::<Vec<_>>(),
        vec![
            EffectParameter::Host,
            EffectParameter::Port,
            EffectParameter::Protocol
        ]
    );

    let body = function(
        0,
        "connect",
        vec![
            string_const(0, "db.internal"),
            int_const(1, 5432),
            call_with_args(2, 100, vec![TirValueId(0), TirValueId(1)]),
        ],
    );
    let values = crate::facts::FunctionValueIndex::new(&body);
    let callsite = TirLocation {
        function: TirFunctionId(0),
        block: TirBlockId(0),
        instruction_index: 2,
    };
    let resources =
        resource_facts_for_binding(&binding, &values, &[TirValueId(0), TirValueId(1)], callsite);

    assert_eq!(
        resources
            .iter()
            .map(|resource| resource.parameter)
            .collect::<Vec<_>>(),
        vec![
            EffectParameter::Host,
            EffectParameter::Port,
            EffectParameter::Protocol
        ]
    );
    assert_eq!(
        resources[0].knowledge,
        ResourceKnowledge::Exact(ResourceValue::Text("db.internal".to_owned()))
    );
    assert_eq!(
        resources[1].knowledge,
        ResourceKnowledge::Exact(ResourceValue::Port(5432))
    );
    assert_eq!(resources[2].knowledge, ResourceKnowledge::Unknown);
}

#[test]
fn duplicate_and_invalid_synthetic_bindings_are_rejected() {
    assert_eq!(
        ExternOperationBinding::new(
            EffectOperation::NetConnect,
            [
                ResourceBinding::argument(EffectParameter::Host, 0),
                ResourceBinding::argument(EffectParameter::Host, 1),
            ],
        ),
        Err(BindingError::DuplicateParameter {
            operation: EffectOperation::NetConnect,
            parameter: EffectParameter::Host,
        })
    );

    assert_eq!(
        ExternOperationBinding::new(
            EffectOperation::FsRead,
            [ResourceBinding::argument(EffectParameter::Destination, 0)],
        ),
        Err(BindingError::InvalidParameter {
            operation: EffectOperation::FsRead,
            parameter: EffectParameter::Destination,
        })
    );

    assert_eq!(
        ExternOperationBinding::new(
            EffectOperation::IoWrite,
            [ResourceBinding::unknown(EffectParameter::Host)],
        ),
        Err(BindingError::InvalidParameter {
            operation: EffectOperation::IoWrite,
            parameter: EffectParameter::Host,
        })
    );

    // Zero resources remain valid for an operation that declares none.
    assert!(ExternOperationBinding::new(
        EffectOperation::ProcessExit,
        std::iter::empty::<ResourceBinding>(),
    )
    .is_ok());
}

#[test]
fn missing_bound_arguments_are_recorded_as_unknown() {
    let binding = ExternOperationBinding::new(
        EffectOperation::NetConnect,
        [
            ResourceBinding::argument(EffectParameter::Host, 0),
            ResourceBinding::argument(EffectParameter::Port, 1),
            ResourceBinding::unknown(EffectParameter::Protocol),
        ],
    )
    .unwrap();

    let body = function(
        0,
        "connect",
        vec![
            string_const(0, "db.internal"),
            call_with_args(1, 100, vec![TirValueId(0)]),
        ],
    );
    let values = crate::facts::FunctionValueIndex::new(&body);
    let callsite = TirLocation {
        function: TirFunctionId(0),
        block: TirBlockId(0),
        instruction_index: 1,
    };
    let resources = resource_facts_for_binding(&binding, &values, &[TirValueId(0)], callsite);

    assert_eq!(resources.len(), 3);
    assert_eq!(
        resources[0].knowledge,
        ResourceKnowledge::Exact(ResourceValue::Text("db.internal".to_owned()))
    );
    assert_eq!(resources[1].knowledge, ResourceKnowledge::Unknown);
    assert_eq!(resources[2].knowledge, ResourceKnowledge::Unknown);
}

#[test]
fn one_specialized_fact_is_created_per_classified_native_call() {
    let caller = function(
        0,
        "main",
        vec![
            string_const(0, "input.txt"),
            call_with_args(1, 100, vec![TirValueId(0)]),
            string_const(2, "hello"),
            call_with_args(3, 101, vec![TirValueId(2)]),
            string_const(4, "build/out.txt"),
            string_const(5, "contents"),
            call_with_args(6, 102, vec![TirValueId(4), TirValueId(5)]),
        ],
    );
    let input = module(vec![
        caller,
        extern_function(100, "std.fs.read_text"),
        extern_function(101, "std.io.print"),
        extern_function(102, "std.fs.write_text"),
    ]);
    let analysis = analyze(&input).unwrap();

    let specialized: Vec<_> = analysis
        .local_effect_facts()
        .iter()
        .map(specialized)
        .collect();
    assert_eq!(specialized.len(), 3);
    assert_eq!(
        specialized
            .iter()
            .map(|fact| fact.location.instruction_index)
            .collect::<Vec<_>>(),
        vec![1, 3, 6]
    );
    for fact in &specialized {
        assert_eq!(fact.resources.len(), 1);
    }

    assert_eq!(analyze(&input).unwrap(), analysis);
}

#[test]
fn propagates_specialized_facts_directly_and_transitively() {
    let analysis = analyze(&module(vec![
        function(
            0,
            "root",
            vec![
                call(0, 1),
                string_const(1, "hello"),
                call_with_args(2, 101, vec![TirValueId(1)]),
            ],
        ),
        function(1, "middle", vec![call(0, 2)]),
        function(
            2,
            "leaf",
            vec![
                string_const(0, "input.txt"),
                call_with_args(1, 100, vec![TirValueId(0)]),
            ],
        ),
        extern_function(100, "std.fs.read_text"),
        extern_function(101, "std.io.print"),
    ]))
    .unwrap();

    let root = analysis.specialized_summary(TirFunctionId(0)).unwrap();
    assert_eq!(root.direct.len(), 1);
    assert_eq!(root.transitive.len(), 1);

    let direct = specialized(&root.direct[0]);
    assert_eq!(direct.operation, EffectOperation::IoWrite);
    assert_eq!(direct.location.function, TirFunctionId(0));

    let read = &root.transitive[0];
    assert_eq!(specialized(read).operation, EffectOperation::FsRead);
    assert_eq!(read.location().function, TirFunctionId(2));
    assert_eq!(
        *resource_knowledge(specialized(read), EffectParameter::Source),
        ResourceKnowledge::Exact(ResourceValue::Text("input.txt".to_owned()))
    );

    let middle = analysis.specialized_summary(TirFunctionId(1)).unwrap();
    assert!(middle.direct.is_empty());
    assert_eq!(middle.transitive, root.transitive);

    let leaf = analysis.specialized_summary(TirFunctionId(2)).unwrap();
    assert_eq!(leaf.direct, root.transitive);
    assert!(leaf.transitive.is_empty());

    let direct_witness = analysis
        .specialized_witness(TirFunctionId(0), &root.direct[0])
        .unwrap();
    assert!(direct_witness.calls.is_empty());
    assert_eq!(&direct_witness.origin, &root.direct[0]);

    let transitive_witness = analysis
        .specialized_witness(TirFunctionId(0), read)
        .unwrap();
    assert_eq!(transitive_witness.calls.len(), 2);
    assert_eq!(transitive_witness.calls[0].caller, TirFunctionId(0));
    assert_eq!(transitive_witness.calls[0].callee, TirFunctionId(1));
    assert_eq!(transitive_witness.calls[1].callee, TirFunctionId(2));
    assert_eq!(&transitive_witness.origin, read);
    assert_eq!(transitive_witness.origin.location(), read.location());
}

#[test]
fn diamond_graphs_deduplicate_origins_and_pick_shortest_witness() {
    let input = module(vec![
        function(0, "root", vec![call(0, 1), call(1, 2)]),
        function(1, "left", vec![call(0, 3)]),
        function(2, "right", vec![call(0, 3)]),
        function(
            3,
            "leaf",
            vec![
                string_const(0, "shared.txt"),
                call_with_args(1, 100, vec![TirValueId(0)]),
            ],
        ),
        extern_function(100, "std.fs.read_text"),
    ]);
    let analysis = analyze(&input).unwrap();

    let root = analysis.specialized_summary(TirFunctionId(0)).unwrap();
    assert!(root.direct.is_empty());
    assert_eq!(root.transitive.len(), 1);
    let origin = root.transitive[0].clone();
    assert_eq!(specialized(&origin).operation, EffectOperation::FsRead);

    for function in [1, 2] {
        let summary = analysis
            .specialized_summary(TirFunctionId(function))
            .unwrap();
        assert_eq!(summary.direct.len(), 0);
        assert_eq!(summary.transitive, vec![origin.clone()]);
    }

    let witness = analysis
        .specialized_witness(TirFunctionId(0), &origin)
        .unwrap();
    assert_eq!(witness.calls.len(), 2);
    assert_eq!(witness.calls[0].caller, TirFunctionId(0));
    // Lower callee function ID wins the deterministic shortest-path tie.
    assert_eq!(witness.calls[0].callee, TirFunctionId(1));
    assert_eq!(witness.calls[1].caller, TirFunctionId(1));
    assert_eq!(witness.calls[1].callee, TirFunctionId(3));
    assert_eq!(&witness.origin, &origin);

    assert_eq!(analyze(&input).unwrap(), analysis);
}

#[test]
fn recursive_and_mutually_recursive_summaries_terminate() {
    let analysis = analyze(&module(vec![
        function(
            0,
            "self_recursive",
            vec![
                string_const(0, "self.txt"),
                call_with_args(1, 100, vec![TirValueId(0)]),
                call(2, 0),
            ],
        ),
        function(
            1,
            "left",
            vec![
                string_const(0, "left.txt"),
                call_with_args(1, 101, vec![TirValueId(0)]),
                call(2, 2),
            ],
        ),
        function(
            2,
            "right",
            vec![
                string_const(0, "right.txt"),
                call_with_args(1, 102, vec![TirValueId(0)]),
                call(2, 1),
            ],
        ),
        function(3, "root", vec![call(0, 1)]),
        extern_function(100, "std.fs.read_text"),
        extern_function(101, "std.fs.write_text"),
        extern_function(102, "std.fs.write_text"),
    ]))
    .unwrap();

    let self_recursive = analysis.specialized_summary(TirFunctionId(0)).unwrap();
    assert_eq!(self_recursive.direct.len(), 1);
    assert!(self_recursive.transitive.is_empty());
    let self_witness = analysis
        .specialized_witness(TirFunctionId(0), &self_recursive.direct[0])
        .unwrap();
    assert!(self_witness.calls.is_empty());

    let left = analysis.specialized_summary(TirFunctionId(1)).unwrap();
    let right = analysis.specialized_summary(TirFunctionId(2)).unwrap();
    assert_eq!(left.direct.len(), 1);
    assert_eq!(left.transitive, right.direct);
    assert_eq!(right.direct.len(), 1);
    assert_eq!(right.transitive, left.direct);

    let root = analysis.specialized_summary(TirFunctionId(3)).unwrap();
    assert_eq!(
        root.transitive,
        vec![left.direct[0].clone(), right.direct[0].clone()]
    );

    let right_origin = right.direct[0].clone();
    let witness = analysis
        .specialized_witness(TirFunctionId(3), &right_origin)
        .unwrap();
    assert_eq!(witness.calls.len(), 2);
    assert_eq!(witness.calls[0].callee, TirFunctionId(1));
    assert_eq!(witness.calls[1].callee, TirFunctionId(2));
    assert_eq!(&witness.origin, &right_origin);
}

#[test]
fn distinct_origins_with_equal_resources_are_not_merged() {
    let analysis = analyze(&module(vec![
        function(0, "root", vec![call(0, 1), call(1, 2)]),
        function(
            1,
            "first",
            vec![
                string_const(0, "same.txt"),
                call_with_args(1, 100, vec![TirValueId(0)]),
            ],
        ),
        function(
            2,
            "second",
            vec![
                string_const(0, "same.txt"),
                call_with_args(1, 100, vec![TirValueId(0)]),
            ],
        ),
        extern_function(100, "std.fs.write_text"),
    ]))
    .unwrap();

    let root = analysis.specialized_summary(TirFunctionId(0)).unwrap();
    assert!(root.direct.is_empty());
    assert_eq!(root.transitive.len(), 2);
    assert_ne!(root.transitive[0].location(), root.transitive[1].location());
    assert_eq!(
        root.transitive
            .iter()
            .map(LocalEffectFact::location)
            .collect::<Vec<_>>(),
        vec![
            TirLocation {
                function: TirFunctionId(1),
                block: TirBlockId(0),
                instruction_index: 1,
            },
            TirLocation {
                function: TirFunctionId(2),
                block: TirBlockId(0),
                instruction_index: 1,
            },
        ]
    );

    for origin in &root.transitive {
        let fact = specialized(origin);
        assert_eq!(fact.operation, EffectOperation::FsWrite);
        assert_eq!(
            *resource_knowledge(fact, EffectParameter::Destination),
            ResourceKnowledge::Exact(ResourceValue::Text("same.txt".to_owned()))
        );
    }

    for (origin, callee) in root
        .transitive
        .iter()
        .zip([TirFunctionId(1), TirFunctionId(2)])
    {
        let witness = analysis
            .specialized_witness(TirFunctionId(0), origin)
            .unwrap();
        assert_eq!(witness.calls.len(), 1);
        assert_eq!(witness.calls[0].callee, callee);
        assert_eq!(&witness.origin, origin);
    }
}

#[test]
fn analysis_exposes_canonical_classification_metadata_for_direct_facts() {
    let input = module(vec![
        function(
            0,
            "main",
            vec![
                string_const(0, "input.txt"),
                call_with_args(1, 100, vec![TirValueId(0)]),
                string_const(2, "output.txt"),
                string_const(3, "contents"),
                call_with_args(4, 101, vec![TirValueId(2), TirValueId(3)]),
                string_const(5, "stdout"),
                call_with_args(6, 102, vec![TirValueId(5)]),
                string_const(7, "."),
                call_with_args(8, 103, vec![TirValueId(7)]),
            ],
        ),
        extern_function(100, "std.fs.read_text"),
        extern_function(101, "std.fs.write_text"),
        extern_function(102, "std.io.print"),
        extern_function(103, "std.fs.exists"),
    ]);
    let analysis = analyze(&input).unwrap();
    let facts = analysis.local_effect_facts();
    assert_eq!(facts.len(), 4);

    let read = facts
        .iter()
        .find(|fact| fact.operation() == Some(EffectOperation::FsRead))
        .expect("read fact");
    assert_eq!(read.capability(), Capability::Io);
    assert_eq!(read.family_membership(EffectFamily::Fs), Some(true));
    assert_eq!(read.family_membership(EffectFamily::Net), Some(false));
    assert_eq!(read.category_membership(EffectCategory::IoRead), Some(true));
    assert_eq!(
        read.category_membership(EffectCategory::IoWrite),
        Some(false)
    );
    assert_eq!(
        resource_knowledge(specialized(read), EffectParameter::Source),
        &ResourceKnowledge::Exact(ResourceValue::Text("input.txt".to_owned()))
    );

    let write = facts
        .iter()
        .find(|fact| fact.operation() == Some(EffectOperation::FsWrite))
        .expect("write fact");
    assert_eq!(write.capability(), Capability::Io);
    assert_eq!(write.family_membership(EffectFamily::Fs), Some(true));
    assert_eq!(write.family_membership(EffectFamily::Net), Some(false));
    assert_eq!(
        write.category_membership(EffectCategory::IoRead),
        Some(false)
    );
    assert_eq!(
        write.category_membership(EffectCategory::IoWrite),
        Some(true)
    );
    assert_eq!(
        resource_knowledge(specialized(write), EffectParameter::Destination),
        &ResourceKnowledge::Exact(ResourceValue::Text("output.txt".to_owned()))
    );

    let print = facts
        .iter()
        .find(|fact| fact.operation() == Some(EffectOperation::IoWrite))
        .expect("print fact");
    assert_eq!(print.capability(), Capability::Io);
    assert_eq!(print.family_membership(EffectFamily::Fs), Some(false));
    assert_eq!(print.family_membership(EffectFamily::Net), Some(false));
    assert_eq!(
        print.category_membership(EffectCategory::IoRead),
        Some(false)
    );
    assert_eq!(
        print.category_membership(EffectCategory::IoWrite),
        Some(true)
    );
    assert_eq!(
        resource_knowledge(specialized(print), EffectParameter::Stream),
        &ResourceKnowledge::Unknown
    );

    let exists = facts
        .iter()
        .find(|fact| {
            matches!(
                fact,
                LocalEffectFact::UnclassifiedOperation(fact)
                    if fact.extern_name == "std.fs.exists"
            )
        })
        .expect("exists fact");
    assert_eq!(exists.operation(), None);
    assert_eq!(exists.capability(), Capability::Io);
    for family in EffectFamily::ALL {
        assert_eq!(exists.family_membership(family), None, "{family:?}");
    }
    for category in EffectCategory::ALL {
        assert_eq!(exists.category_membership(category), None, "{category:?}");
    }

    let summary = analysis.summary(TirFunctionId(0)).unwrap();
    assert_eq!(
        summary.required,
        capabilities(&[Capability::Alloc, Capability::Io])
    );
}

#[test]
fn propagated_diamond_and_recursive_facts_preserve_classification_metadata() {
    let analysis = analyze(&module(vec![
        function(0, "root", vec![call(0, 1), call(1, 2)]),
        function(1, "left", vec![call(0, 3)]),
        function(2, "right", vec![call(0, 3)]),
        function(
            3,
            "leaf",
            vec![
                string_const(0, "leaf.txt"),
                call_with_args(1, 100, vec![TirValueId(0)]),
                call(2, 3),
            ],
        ),
        extern_function(100, "std.fs.read_text"),
    ]))
    .unwrap();

    let leaf = analysis.specialized_summary(TirFunctionId(3)).unwrap();
    assert_eq!(leaf.direct.len(), 1);
    assert_eq!(leaf.transitive, Vec::new());

    let root = analysis.specialized_summary(TirFunctionId(0)).unwrap();
    assert_eq!(root.transitive.len(), 1);
    let origin = &root.transitive[0];
    assert_eq!(origin, &leaf.direct[0]);
    assert_eq!(origin.operation(), Some(EffectOperation::FsRead));
    assert_eq!(origin.capability(), Capability::Io);
    assert_eq!(origin.family_membership(EffectFamily::Fs), Some(true));
    assert_eq!(origin.family_membership(EffectFamily::Net), Some(false));
    assert_eq!(
        origin.category_membership(EffectCategory::IoRead),
        Some(true)
    );
    assert_eq!(
        origin.category_membership(EffectCategory::IoWrite),
        Some(false)
    );
    let fact = specialized(origin);
    assert_eq!(fact.extern_id, TirFunctionId(100));
    assert_eq!(fact.extern_name, "std.fs.read_text");
    assert_eq!(
        resource_knowledge(fact, EffectParameter::Source),
        &ResourceKnowledge::Exact(ResourceValue::Text("leaf.txt".to_owned()))
    );

    let witness = analysis
        .specialized_witness(TirFunctionId(0), origin)
        .expect("diamond witness");
    assert_eq!(witness.calls.len(), 2);
    assert_eq!(witness.calls[0].callee, TirFunctionId(1));
    assert_eq!(witness.calls[1].callee, TirFunctionId(3));
    assert_eq!(&witness.origin, origin);
    assert_eq!(
        witness.origin.family_membership(EffectFamily::Fs),
        Some(true)
    );

    assert_eq!(
        analysis.summary(TirFunctionId(0)).unwrap().required,
        capabilities(&[Capability::Alloc, Capability::Io])
    );
}

#[test]
fn net_transfer_operations_remain_taxonomy_only() {
    for (operation, family, category) in [
        (
            EffectOperation::NetSend,
            EffectFamily::Net,
            EffectCategory::IoWrite,
        ),
        (
            EffectOperation::NetReceive,
            EffectFamily::Net,
            EffectCategory::IoRead,
        ),
    ] {
        assert_eq!(operation.parent(), Capability::Io);
        assert_eq!(operation.family(), Some(family));
        assert_eq!(operation.categories(), &[category]);
        assert!(operation.valid_parameter_keys().is_empty());
    }

    assert!(known_externs().all(|extern_definition| {
        extern_definition.operation != Some(EffectOperation::NetSend)
            && extern_definition.operation != Some(EffectOperation::NetReceive)
    }));
    assert!(
        known_externs().all(|extern_definition| !extern_definition.name.starts_with("std.net."))
    );

    let called = module(vec![
        function(0, "main", vec![call(0, 100)]),
        extern_function(100, "std.net.send"),
    ]);
    assert!(matches!(
        analyze(&called),
        Err(CapabilityAnalysisError::UnclassifiedExtern { name, .. })
            if name == "std.net.send"
    ));
}

#[test]
fn propagated_unknown_resources_are_not_specialized_from_caller_arguments() {
    let mut callee = function(
        1,
        "reader",
        vec![call_with_args(1, 100, vec![TirValueId(0)])],
    );
    callee.params = vec![(TirValueId(0), TirType::String)];

    let analysis = analyze(&module(vec![
        function(
            0,
            "caller",
            vec![
                string_const(0, "caller-argument.txt"),
                call_with_args(1, 1, vec![TirValueId(0)]),
            ],
        ),
        callee,
        extern_function(100, "std.fs.read_text"),
    ]))
    .unwrap();

    let callee_summary = analysis.specialized_summary(TirFunctionId(1)).unwrap();
    assert_eq!(callee_summary.direct.len(), 1);
    let origin = callee_summary.direct[0].clone();
    assert_eq!(
        *resource_knowledge(specialized(&origin), EffectParameter::Source),
        ResourceKnowledge::Unknown
    );

    let caller_summary = analysis.specialized_summary(TirFunctionId(0)).unwrap();
    assert_eq!(caller_summary.transitive, vec![origin.clone()]);
    assert_ne!(
        *resource_knowledge(
            specialized(&caller_summary.transitive[0]),
            EffectParameter::Source
        ),
        ResourceKnowledge::Exact(ResourceValue::Text("caller-argument.txt".to_owned()))
    );

    let witness = analysis
        .specialized_witness(TirFunctionId(0), &origin)
        .unwrap();
    assert_eq!(witness.calls.len(), 1);
    assert_eq!(witness.origin.location().function, TirFunctionId(1));
}

#[test]
fn propagation_preserves_exact_and_unknown_resource_knowledge() {
    let mut dynamic = function(
        2,
        "dynamic",
        vec![call_with_args(1, 100, vec![TirValueId(0)])],
    );
    dynamic.params = vec![(TirValueId(0), TirType::String)];

    let analysis = analyze(&module(vec![
        function(0, "root", vec![call(0, 1), call(1, 2)]),
        function(
            1,
            "exact",
            vec![
                string_const(0, "exact.txt"),
                call_with_args(1, 100, vec![TirValueId(0)]),
            ],
        ),
        dynamic,
        extern_function(100, "std.fs.read_text"),
    ]))
    .unwrap();

    let root = analysis.specialized_summary(TirFunctionId(0)).unwrap();
    assert_eq!(root.transitive.len(), 2);
    assert_eq!(root.transitive[0].location().function, TirFunctionId(1));
    assert_eq!(root.transitive[1].location().function, TirFunctionId(2));

    let exact = specialized(&root.transitive[0]);
    assert_eq!(exact.operation, EffectOperation::FsRead);
    assert_eq!(resource_parameters(exact), vec![EffectParameter::Source]);
    assert_eq!(
        *resource_knowledge(exact, EffectParameter::Source),
        ResourceKnowledge::Exact(ResourceValue::Text("exact.txt".to_owned()))
    );

    let unknown = specialized(&root.transitive[1]);
    assert_eq!(unknown.operation, EffectOperation::FsRead);
    assert_eq!(resource_parameters(unknown), vec![EffectParameter::Source]);
    assert_eq!(
        *resource_knowledge(unknown, EffectParameter::Source),
        ResourceKnowledge::Unknown
    );
}

#[test]
fn propagates_unclassified_operations_under_known_capabilities() {
    let analysis = analyze(&module(vec![
        function(
            0,
            "root",
            vec![
                string_const(0, "hello"),
                call_with_args(1, 101, vec![TirValueId(0)]),
                call(2, 1),
            ],
        ),
        function(
            1,
            "exists",
            vec![
                string_const(0, "."),
                call_with_args(1, 100, vec![TirValueId(0)]),
            ],
        ),
        extern_function(100, "std.fs.exists"),
        extern_function(101, "std.io.print"),
    ]))
    .unwrap();

    let exists = analysis.specialized_summary(TirFunctionId(1)).unwrap();
    assert_eq!(exists.direct.len(), 1);
    assert!(matches!(
        &exists.direct[0],
        LocalEffectFact::UnclassifiedOperation(fact)
            if fact.capability == Capability::Io
                && fact.extern_name == "std.fs.exists"
    ));

    let root = analysis.specialized_summary(TirFunctionId(0)).unwrap();
    assert_eq!(root.direct.len(), 1);
    assert_eq!(
        specialized(&root.direct[0]).operation,
        EffectOperation::IoWrite
    );
    assert_eq!(root.transitive, exists.direct);
    assert!(matches!(
        &root.transitive[0],
        LocalEffectFact::UnclassifiedOperation(fact)
            if fact.capability == Capability::Io
    ));

    assert_eq!(
        analysis.summary(TirFunctionId(0)).unwrap().required,
        capabilities(&[Capability::Alloc, Capability::Io])
    );

    let witness = analysis
        .specialized_witness(TirFunctionId(0), &root.transitive[0])
        .unwrap();
    assert_eq!(witness.calls.len(), 1);
    assert_eq!(witness.calls[0].callee, TirFunctionId(1));
    assert_eq!(witness.origin.location().function, TirFunctionId(1));
}

#[test]
fn specialized_propagation_does_not_change_broad_summaries_or_witnesses() {
    let analysis = analyze(&module(vec![
        function(0, "root", vec![call(0, 1)]),
        function(
            1,
            "leaf",
            vec![
                string_const(0, "input.txt"),
                call_with_args(1, 100, vec![TirValueId(0)]),
                string_const(2, "out.txt"),
                string_const(3, "contents"),
                call_with_args(4, 101, vec![TirValueId(2), TirValueId(3)]),
            ],
        ),
        extern_function(100, "std.fs.read_text"),
        extern_function(101, "std.fs.write_text"),
    ]))
    .unwrap();

    let both = capabilities(&[Capability::Alloc, Capability::Io]);
    let root = analysis.summary(TirFunctionId(0)).unwrap();
    assert_eq!(root.direct, CapabilitySet::empty());
    assert_eq!(root.required, both);
    let leaf = analysis.summary(TirFunctionId(1)).unwrap();
    assert_eq!(leaf.direct, both);
    assert_eq!(leaf.required, both);

    let io_witness = analysis.witness(TirFunctionId(0), Capability::Io).unwrap();
    assert_eq!(io_witness.calls.len(), 1);
    assert_eq!(io_witness.calls[0].callee, TirFunctionId(1));
    assert_eq!(io_witness.origin.location.function, TirFunctionId(1));
    assert_eq!(io_witness.origin.location.instruction_index, 1);

    let alloc_witness = analysis
        .witness(TirFunctionId(0), Capability::Alloc)
        .unwrap();
    assert_eq!(alloc_witness.calls.len(), 1);
    assert_eq!(alloc_witness.origin.location.function, TirFunctionId(1));
    assert_eq!(alloc_witness.origin.location.instruction_index, 0);

    assert_eq!(
        analysis
            .specialized_summary(TirFunctionId(0))
            .unwrap()
            .transitive
            .len(),
        2
    );
}

#[test]
fn specialized_witness_uses_deterministic_callsite_tie_breaks() {
    let analysis = analyze(&module(vec![
        function_with_blocks(
            0,
            "root",
            // Store the numerically later block first; the canonical order is
            // block 3 before block 9.
            vec![
                block(9, vec![call(0, 1)]),
                block(3, vec![call(0, 1), call(1, 1)]),
            ],
        ),
        function(
            1,
            "leaf",
            vec![
                string_const(0, "leaf.txt"),
                call_with_args(1, 100, vec![TirValueId(0)]),
            ],
        ),
        extern_function(100, "std.fs.read_text"),
    ]))
    .unwrap();

    let root = analysis.specialized_summary(TirFunctionId(0)).unwrap();
    assert_eq!(root.transitive.len(), 1);
    let witness = analysis
        .specialized_witness(TirFunctionId(0), &root.transitive[0])
        .unwrap();
    assert_eq!(witness.calls.len(), 1);
    assert_eq!(witness.calls[0].callsite.block, TirBlockId(3));
    assert_eq!(witness.calls[0].callsite.instruction_index, 0);
}

#[test]
#[should_panic(expected = "compiler-owned extern operation binding must be valid")]
fn invalid_static_binding_is_a_compiler_error() {
    let _ = static_binding(
        EffectOperation::FsRead,
        [ResourceBinding::argument(EffectParameter::Destination, 0)],
    );
}

#[test]
fn specialized_witness_prefers_a_shorter_path_to_the_same_origin() {
    let analysis = analyze(&module(vec![
        function(0, "root", vec![call(0, 1), call(1, 2)]),
        function(1, "middle", vec![call(0, 2)]),
        function(
            2,
            "leaf",
            vec![
                string_const(0, "shortest.txt"),
                call_with_args(1, 100, vec![TirValueId(0)]),
            ],
        ),
        extern_function(100, "std.fs.read_text"),
    ]))
    .unwrap();

    let root = analysis.specialized_summary(TirFunctionId(0)).unwrap();
    assert_eq!(root.transitive.len(), 1);
    let witness = analysis
        .specialized_witness(TirFunctionId(0), &root.transitive[0])
        .unwrap();
    assert_eq!(witness.calls.len(), 1);
    assert_eq!(witness.calls[0].callee, TirFunctionId(2));
    assert_eq!(witness.origin.location().function, TirFunctionId(2));
}
