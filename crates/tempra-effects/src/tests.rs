use crate::{analyze, Capability, CapabilityAnalysisError, CapabilitySet, DirectCapabilitySource};
use std::collections::HashMap;
use tempra_tir::ir::{
    TirBlock, TirBlockId, TirConst, TirFunction, TirFunctionId, TirInstr, TirKey, TirModule,
    TirTerminator, TirType, TirValueId,
};

fn block(id: usize, instrs: Vec<TirInstr>) -> TirBlock {
    TirBlock {
        id: TirBlockId(id),
        instrs,
        terminator: Some(TirTerminator::Ret(None)),
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
