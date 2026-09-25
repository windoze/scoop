use super::*;

// This is a core-bootstrap structural test, not evidence that the ordinary
// imported-core path accepts pointer source names or grants access authority.
#[test]
fn default_type_access_pointer_demands_use_real_bootstrap_default_templates() {
    let parsed = pointer_sources();
    let output = lower_core_bootstrap(&parsed).unwrap();
    let canonical = hir::CanonicalHirFoundation::from_modules(
        &output.export,
        &output.local,
        &output.native_boundary_types,
    )
    .unwrap();
    let decoded: hir::DecodedHirFoundation =
        decode_canonical(&encode(&canonical).unwrap()).unwrap();
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    pending
        .register_authority(scoop_identity::ConeIdentity::CORE)
        .unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let templates = ["raw", "native", "genericPointer", "combinedPointers"]
        .into_iter()
        .map(|name| {
            let function = output
                .export
                .functions
                .iter()
                .find(|(_, f)| f.name == name)
                .unwrap()
                .0;
            (
                name,
                hir::DefaultSourceBodyProductionV1::from_export_hir(
                    &output.export,
                    hir::ExportParameterOwner::Function(function),
                    1,
                )
                .unwrap()
                .into_source_template()
                .unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        outline(&identities, &templates),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/type-access-pointers.snap"
        ))
    );
    resources::assert_borrowed_roots(&templates);
}

fn pointer_sources() -> ast::CurrentConeParsedSources {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/type-access-pointers.scoop"
    ));
    let core = crate::tests::core_source_identity("src/core.scoop");
    let pointer = crate::tests::core_source_identity("src/type-access-pointers.scoop");
    ast::CurrentConeParsedSources::try_new(
        ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
            ast::IdentifiedParsedSource::new(core.clone(), complete_core_file()),
            vec![ast::IdentifiedParsedSource::new(
                pointer.clone(),
                scoop_parser::parse(text).unwrap(),
            )],
        ))
        .unwrap(),
        ast::NonEmptyVec::new(
            ast::CurrentSourceText::new(core.clone(), String::new()),
            vec![ast::CurrentSourceText::new(
                pointer.clone(),
                text.to_owned(),
            )],
        ),
        ast::NonEmptyVec::new(
            ast::CurrentSourceDiagnosticContext::new(
                core,
                std::path::PathBuf::from("src/core.scoop"),
            ),
            vec![ast::CurrentSourceDiagnosticContext::new(
                pointer,
                std::path::PathBuf::from("src/type-access-pointers.scoop"),
            )],
        ),
    )
    .unwrap()
}
