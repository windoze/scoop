#[test]
fn atomic_methods_normalize_through_source_generics_and_reference_types() {
    let source = [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m33-atomics/operations/src/main.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m33-atomics/operations/src/scalars.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m33-atomics/operations/src/references.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m33-atomics/operations/src/evaluation.scoop"
        )),
    ]
    .join("\n");
    let output = super::super::core::lower_with_sysroot(&source).unwrap();
    let module = scoop_mir_lower::lower(&output.local).unwrap();
    let dump = scoop_mir::dump(&module);
    for operation in ["AtomicLoad", "AtomicStore", "AtomicRmw", "AtomicCmpXchg"] {
        assert!(
            dump.contains(operation),
            "missing normalized operation {operation}"
        );
    }
    for kind in ["Int", "Long", "Boolean", "Reference"] {
        assert!(
            dump.contains(&format!("AtomicLoad SeqCst {kind}\n")),
            "missing atomic {kind} read"
        );
    }
}
