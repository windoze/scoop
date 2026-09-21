use super::*;
use scoop_identity::{ConeIdentity, CoreBuiltinNominal, RequestedConeKind};

#[test]
fn core_declarations_retain_shared_dependency_selections_and_routes() {
    let mut core = trusted_core();
    let provider = DependencyFunctionFixture::new(
        "core-helper",
        "run",
        SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
    );
    let foundation =
        core.import_dependency_foundation(&provider.coordinate, &provider.foundation, 61);
    let aliases = empty_alias_expansions();
    let parsed = core_sources();
    let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
        ConeIdentity::CORE,
        vec![scoop_hir::DirectImportedProviderInput::from_validated(
            certificate(&provider.coordinate, 61),
            &foundation,
            &provider.interface,
            &aliases,
        )],
        Vec::new(),
    )
    .unwrap();
    let input = CurrentConeSources::try_new(
        &parsed,
        crate::CoreProtocolInput::CurrentDeclarations,
        &world,
    )
    .unwrap();
    let output = lower_current_cone(RequestedConeKind::Library, &input).unwrap();
    assert!(matches!(
        output.output().export.core_protocols,
        scoop_hir::CoreProtocols::Defined(_)
    ));
    assert!(matches!(
        output.output().local.materialization(),
        scoop_hir::LocalConcreteMaterializationContract::CoreShapeSupport(_)
    ));
    assert_eq!(
        output.imported_dependencies().consumer(),
        ConeIdentity::CORE
    );
    assert_eq!(output.imported_dependencies().callable_count(), 1);
    assert_eq!(output.concrete_dependency_witness_uses().len(), 1);
    assert_eq!(output.output().local.imported_dependency_callables.len(), 1);
    let dump = scoop_hir::dump(&output.output().export);
    assert_eq!(dump.matches("ImportedDependencyCall #0").count(), 3);
    assert_core_hir_snapshot(&dump);
    let selected = output.imported_dependencies().callables().next().unwrap();
    assert_eq!(
        selected.interface().declaration(),
        provider.interface.callable_interfaces().records()[0].declaration()
    );
    let canonical = scoop_hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
    let mut authority = scoop_hir::CrossConeHirProductionAuthority::new(
        &canonical,
        &output.output().export.public_export_bindings,
        &world,
    );
    let public = scoop_hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(
        &output,
        &[],
        &mut authority,
    )
    .unwrap();
    assert!(!public.external_references().records().is_empty());
    drop(world);
    assert_eq!(output.imported_dependencies().callable_count(), 1);
}

#[test]
fn protocol_origin_and_semantic_world_must_match_current_sources() {
    let core = trusted_core();
    let parsed = core_sources();
    let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
        ConeIdentity::CORE,
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let imported = core.foundation.import_core_inputs(&core.interface).unwrap();
    assert!(matches!(
        CurrentConeSources::try_new(&parsed, imported, &world),
        Err(crate::CurrentConeSourceError::CoreImportsOwnProtocols)
    ));
    let ordinary = parsed_ordinary(file(Vec::new()));
    let other_world = core.world(ordinary.cone());
    assert!(matches!(
        CurrentConeSources::try_new(
            &ordinary,
            crate::CoreProtocolInput::CurrentDeclarations,
            &other_world
        ),
        Err(crate::CurrentConeSourceError::CurrentProtocolsInOrdinaryCone(_))
    ));
    assert!(matches!(
        CurrentConeSources::try_new(
            &parsed,
            crate::CoreProtocolInput::CurrentDeclarations,
            &other_world
        ),
        Err(crate::CurrentConeSourceError::SemanticWorldCurrentConeMismatch { .. })
    ));
}

fn core_sources() -> scoop_ast::CurrentConeParsedSources {
    core_sources_with_calls(include_str!(
        "../../../../../tests/fixtures/core-library/dependency-calls.scoop"
    ))
}

fn core_sources_with_calls(text: &str) -> scoop_ast::CurrentConeParsedSources {
    use scoop_ast::{
        AllParsedSources, CurrentConeParsedSources, CurrentSourceDiagnosticContext,
        CurrentSourceText, IdentifiedParsedSource, NonEmptyVec,
    };
    let locals = include_str!("../../../../../tests/fixtures/core-library/dependency-locals.scoop");
    let sources = [
        ("src/core.scoop", "", crate::tests::complete_core_file()),
        (
            "src/dependency-calls.scoop",
            text,
            scoop_parser::parse(text).unwrap(),
        ),
        (
            "src/dependency-locals.scoop",
            locals,
            scoop_parser::parse(locals).unwrap(),
        ),
    ];
    let mut parsed = Vec::new();
    let mut texts = Vec::new();
    let mut diagnostics = Vec::new();
    for (path, text, source) in sources {
        let identity = crate::tests::core_source_identity(path);
        parsed.push(IdentifiedParsedSource::new(identity.clone(), source));
        texts.push(CurrentSourceText::new(identity.clone(), text.to_owned()));
        diagnostics.push(CurrentSourceDiagnosticContext::new(identity, path.into()));
    }
    CurrentConeParsedSources::try_new(
        AllParsedSources::try_new(NonEmptyVec::new(parsed.remove(0), parsed)).unwrap(),
        NonEmptyVec::new(texts.remove(0), texts),
        NonEmptyVec::new(diagnostics.remove(0), diagnostics),
    )
    .unwrap()
}

#[test]
fn core_missing_import_reports_the_shared_source_diagnostic() {
    let text = include_str!("../../../../../tests/fixtures/core-library/dependency-missing.scoop");
    let sources = core_sources_with_calls(text);
    let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
        ConeIdentity::CORE,
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let input = CurrentConeSources::try_new(
        &sources,
        crate::CoreProtocolInput::CurrentDeclarations,
        &world,
    )
    .unwrap();
    let errors = lower_current_cone(RequestedConeKind::Library, &input)
        .err()
        .expect("an unavailable import must fail before body lowering");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].file, 1);
    assert_eq!(errors[0].span, Some(scoop_ast::Span { start: 7, end: 29 }));
    assert_eq!(
        errors[0].message,
        "import target is not available in the current compilation unit"
    );
}

fn assert_core_hir_snapshot(dump: &str) {
    let mut selected = String::new();
    let mut keep = false;
    for line in dump.lines() {
        if line.starts_with("  ") && !line.starts_with("   ") {
            keep = line.contains("userCore") || line.contains("LocalTools");
        }
        if keep {
            selected.push_str(line);
            selected.push('\n');
        }
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/core-library/dependency-calls.hir.snap");
    if std::env::var_os("SCOOP_UPDATE_CORE_DEPENDENCY_SNAPSHOTS").is_some() {
        std::fs::write(&path, &selected).unwrap();
    }
    assert_eq!(selected, std::fs::read_to_string(path).unwrap());
}
