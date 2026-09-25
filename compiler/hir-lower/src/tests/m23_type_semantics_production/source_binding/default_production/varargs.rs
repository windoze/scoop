use super::*;

#[test]
fn type_default_production_preserves_vararg_default_and_empty_omission() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-protected-production/default-varargs.scoop"
    ));
    let mut core = complete_core_file();
    core.declarations
        .extend(scoop_parser::parse(source).unwrap().declarations);
    let parsed = crate::tests::m23_ordinary_core_only::support::parsed_core(core);
    let input = crate::CoreBootstrapSources::try_new(&parsed).unwrap();
    let output = crate::lower_core_bootstrap(&input).unwrap();
    let output = hir::DependencyHirOutput::try_new(
        output,
        hir::ImportedDependencySelectionPlan::empty(ConeIdentity::CORE).finish(),
        Vec::new(),
    )
    .unwrap();
    let (_, _, protocols) =
        hir::ProtectedDeclarationSourceProductionV1::from_export_hir(&output.output().export)
            .unwrap()
            .into_parts();
    let inheritance = hir::CanonicalNominalInheritanceInterfacesV1::try_new(Vec::new()).unwrap();
    let defaults = hir::CanonicalProtectedDefaultTemplatesV1::from_dependency_hir(
        &output,
        &protocols,
        &inheritance,
    )
    .unwrap();
    let sources = hir::NominalDefaultSourceProductionV1::from_dependency_hir(&output).unwrap();
    let domains = direct_domains(&output);
    for template in defaults.records() {
        let original = sources.templates().get(template.key()).unwrap();
        assert_source_body(template, original);
        let closure = original
            .bind_reference_occurrences(&WirePath::root())
            .unwrap();
        let mut visitor = Occurrences::new(&closure, &domains[&template.key().owner()]);
        template
            .references()
            .validate_body_closure(
                template.key(),
                template.body(),
                template.locals(),
                template.definition_origin(),
                template.receiver(),
                &mut visitor,
                &WirePath::root(),
            )
            .unwrap();
        assert_eq!(visitor.count, closure.occurrences().len());
    }
    let mut found = [false; 2];
    for protocol in protocols.records() {
        for parameter in protocol.parameters().parameters() {
            match parameter.calling() {
                hir::ProtectedParameterCallingV1::VarargDefault { template, .. } => {
                    let body = defaults.get(*template).unwrap();
                    assert!(matches!(
                        body.body().value().kind(),
                        hir::DefaultExpressionKindV1::ArrayLiteral(_)
                    ));
                    found[0] = true;
                }
                hir::ProtectedParameterCallingV1::VarargEmpty { .. } => found[1] = true,
                _ => {}
            }
        }
    }
    assert_eq!(found, [true; 2]);
}
