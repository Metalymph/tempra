use tempra_interpreter::value::Value;
use tempra_vm::value::VmValue;

#[test]
fn vm_burnin_matches_hir_reference() {
    let cases = [
        (
            include_str!("../../../examples/vm_burnin/int64_call_chain.tmpr"),
            115,
        ),
        (
            include_str!("../../../examples/vm_burnin/int64_nested_loops.tmpr"),
            125,
        ),
        (
            include_str!("../../../examples/vm_burnin/int64_branch_heavy.tmpr"),
            3,
        ),
        (
            include_str!("../../../examples/vm_burnin/int64_mixed_string.tmpr"),
            205,
        ),
        (
            include_str!("../../../examples/vm_burnin/int64_struct.tmpr"),
            300,
        ),
        (
            include_str!("../../../examples/vm_burnin/int64_string.tmpr"),
            50,
        ),
    ];
    for (source, expected) in cases {
        let hir = tempra_driver::run_source_hir(source, &[]).unwrap();
        let vm = tempra_driver::run_source_vm(source, &[]).unwrap();
        assert!(matches!(hir, Value::Int64(actual) if actual == expected));
        assert_eq!(vm, VmValue::Int64(expected));
    }
}
