use super::*;
use scoop_mir as mir;

#[test]
fn intrinsic_interface_entries_retain_exact_signatures_and_effects() {
    let output = bootstrap_sysroot();
    let nominals = hir::CanonicalNominalInterfacesV1::from_export_hir(&output.export).unwrap();
    let classifier =
        hir::NominalExactLeafClassifierV1::try_from_nominal_interfaces(nominals.all_records())
            .unwrap();
    let callables = hir::CanonicalCallableInterfacesV1::from_export_hir(&output.export).unwrap();
    let mut exported = 0;
    for callable in callables.all_declarations() {
        if matches!(
            callable.effects().implementation(),
            hir::CallableImplementationV1::Intrinsic(_)
        ) && classifier.classify_callable(callable).unwrap().is_some()
        {
            exported += 1;
        }
    }
    assert_eq!(
        exported, 113,
        "only intrinsic implementations of published capabilities need entries"
    );
    let module = scoop_mir_lower::lower(&output.local).unwrap();
    assert_eq!(module.validate(), Ok(()));
    let mut count = 0;
    let mut entries = 0;
    for (_, source) in output.local.functions.iter() {
        let hir::concrete::FunctionKind::Intrinsic(intrinsic) = &source.kind else {
            continue;
        };
        let is_add = matches!(
            intrinsic.kind,
            hir::IntrinsicFunctionKind::Integer(hir::IntegerIntrinsicKind::NoGcOperation {
                operation: hir::NoGcIntegerOperation::Add,
                ..
            }) | hir::IntrinsicFunctionKind::Float(hir::FloatIntrinsicKind::Binary {
                operation: hir::FloatBinaryOperator::Add,
                ..
            }) | hir::IntrinsicFunctionKind::PrimitiveBinary(
                hir::PrimitiveBinaryKind::StringConcat
            )
        );
        if is_add {
            assert_eq!(intrinsic.parameters.len(), 2);
            assert_eq!(source.receiver.value_type(), Some(intrinsic.parameters[0]));
            count += 1;
        }
        let Some(entry) = module
            .meta
            .source_callable_materializations
            .iter()
            .find(|entry| entry.materialization() == source.materialization)
        else {
            continue;
        };
        let function = &module.functions[entry.function()];
        assert_eq!(function.params.len(), intrinsic.parameters.len());
        assert_eq!(
            function.gc_effect,
            match source.attributes.gc_effect {
                hir::GcEffect::Managed => mir::GcEffect::Managed,
                hir::GcEffect::NoGc => mir::GcEffect::NoGc,
            }
        );
        entries += 1;
    }
    assert_eq!(count, 11);
    assert_eq!(
        entries, 113,
        "the core publishes all built-in intrinsic capability implementations"
    );
}

#[test]
fn concrete_bound_call_keeps_direct_integer_operation() {
    let output = core::lower_with_sysroot(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m41-add/basic/program.scoop"
    )))
    .unwrap();
    let module = scoop_mir_lower::lower(&output.local).unwrap();
    assert_eq!(module.validate(), Ok(()));
    let (_, source) = output
        .local
        .functions
        .iter()
        .find(|(_, function)| {
            function.name == "add"
                && matches!(
                    output.local.types[function.return_ty].kind,
                    hir::concrete::TypeKind::Integer(hir::IntegerKind::SIGNED_32)
                )
        })
        .expect("the Int bound call is instantiated");
    let entry = module
        .meta
        .source_callable_materializations
        .iter()
        .find(|entry| entry.materialization() == source.materialization)
        .unwrap();
    assert!(
        module.functions[entry.function()]
            .body
            .blocks
            .iter()
            .any(|(_, block)| {
                matches!(&block.terminator, mir::Terminator::Return { value }
            if matches!(value.kind, mir::ExprKind::IntegerBinary { .. }))
            }),
        "the concrete bound call remains a direct integer operation"
    );
}

fn bootstrap_sysroot() -> hir::Output {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sysroot/lib/scoop.core/src");
    let mut paths = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "scoop"))
        .collect::<Vec<_>>();
    paths.sort();
    let text = paths
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    let identity = core_source_identity("src/core.scoop");
    let parsed = ast::CurrentConeParsedSources::try_new(
        ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
            ast::IdentifiedParsedSource::new(identity.clone(), scoop_parser::parse(&text).unwrap()),
            Vec::new(),
        ))
        .unwrap(),
        ast::NonEmptyVec::new(
            ast::CurrentSourceText::new(identity.clone(), text),
            Vec::new(),
        ),
        ast::NonEmptyVec::new(
            ast::CurrentSourceDiagnosticContext::new(identity, std::path::PathBuf::from("<core>")),
            Vec::new(),
        ),
    )
    .unwrap();
    crate::lower_core_bootstrap(&parsed).unwrap()
}
