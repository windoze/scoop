use scoop_hir as hir;
use scoop_mir as mir;

#[test]
fn imported_atomic_constructors_retain_binders_and_default_expressions() {
    use super::m23_ordinary_core_only::support::{parsed_ordinary_text, trusted_core_from_source};
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../sysroot/lib/scoop.core/src/atomics.scoop"
    ));
    let mut declarations = super::complete_core_file();
    declarations
        .declarations
        .extend(scoop_parser::parse(source).unwrap().declarations);
    let core = trusted_core_from_source(declarations, source);
    let parsed = parsed_ordinary_text(
        r#"
        class Payload()
        typealias Counter = AtomicInt
        typealias TextCell = AtomicRef<String>
        fun <T : ref> make(value: T): AtomicRef<T> = AtomicRef(value)
        fun defaults(value: Payload, cell: AtomicRef<Payload> = AtomicRef(value)): AtomicRef<Payload> = cell
        fun integer(): AtomicInt = AtomicInt(17)
        fun wide(): AtomicLong = AtomicLong(23L)
        fun flag(): AtomicBoolean = AtomicBoolean(false)
        fun use(): AtomicRef<Payload> = make(Payload())
        fun aliases() {
            val direct = AtomicInt(17)
            val counter = Counter(23)
            val text = TextCell("value")
        }
    "#,
    );
    let world = core.world(parsed.cone());
    let input = crate::CurrentConeSources::try_new(
        &parsed,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let output =
        crate::lower_current_cone(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let export = &output.output().export;
    let make = export
        .functions
        .iter()
        .find(|(_, function)| function.name == "make")
        .unwrap()
        .1;
    assert!(matches!(
        make.type_params()[0].bounds,
        hir::TypeParamBounds::Ref { .. }
    ));
    let hir::Type::Class(application) = export.types[make.return_ty] else {
        panic!("the imported atomic constructor returns its reference class")
    };
    let hir::ClassApplicationRepresentation::Intrinsic(hir::IntrinsicTypeRepresentation::Atomic(
        hir::AtomicStorage::Reference(value),
    )) = export.class_applications[application].representation
    else {
        panic!("the atomic application retains its reference value type")
    };
    assert_eq!(
        export.types[value],
        hir::Type::Param(make.type_params()[0].id)
    );
    let dump = hir::dump(export);
    for ty in [
        "AtomicInt",
        "AtomicLong",
        "AtomicBoolean",
        "AtomicRef<String>",
    ] {
        assert!(
            dump.contains(&format!("AtomicNew : {ty}\n")),
            "missing imported construction {ty}"
        );
    }
}

#[test]
fn atomic_class_applications_keep_reference_arguments_and_initial_values() {
    let output = super::core::lower_with_sysroot(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m33-atomics/construction/program.scoop"
    )))
    .unwrap();
    let module = scoop_mir_lower::lower(&output.local).unwrap();
    let mut kinds = std::collections::BTreeSet::new();
    for (id, class) in module.classes.iter() {
        if let mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Atomic(
            storage,
        )) = &class.representation
        {
            kinds.insert(storage.kind());
            assert!(mir::array_type(&module, &mir::Type::Class(id)).is_none());
        }
    }
    assert_eq!(
        kinds,
        scoop_identity::AtomicValueKind::ALL
            .iter()
            .copied()
            .collect()
    );
    let dump = mir::dump(&module);
    for ty in [
        "AtomicInt",
        "AtomicLong",
        "AtomicBoolean",
        "AtomicRef<Payload>",
        "AtomicRef<String>",
        "AtomicRef<() -> Int>",
    ] {
        assert!(
            dump.contains(&format!("AtomicNew {ty}\n")),
            "missing allocation for {ty}"
        );
    }
}
