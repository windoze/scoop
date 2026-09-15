use std::path::PathBuf;

use scoop_ast::{
    AllParsedSources, CurrentConeParsedSources, CurrentSourceDiagnosticContext, CurrentSourceText,
    IdentifiedParsedSource, NonEmptyVec,
};
use scoop_identity::{
    ConeIdentity, PendingIdentityValidation, SemanticIdentitySession, SemanticOriginFingerprint,
    SourceIdentity,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::{
    call, complete_core_file, file, fun, fun_expr, int_lit, make_core_public, stmt,
    test_source_identity, ty_named,
};
use crate::{
    CoreBootstrapSources, OrdinaryCoreOnlySources, lower_core_bootstrap, lower_ordinary_core_only,
};

#[test]
fn ordinary_library_lowers_against_imported_core_without_core_sources() {
    let core = trusted_core();
    let ordinary = parsed_ordinary(file(Vec::new()));
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();

    let output = lower_ordinary_core_only(scoop_identity::RequestedConeKind::Library, &input)
        .expect("ordinary library lowering uses imported core authority");

    assert_eq!(output.output().export.source_files.len(), 1);
    assert_eq!(
        output.output().export.source_files[0].identity.cone(),
        ordinary.cone()
    );
    assert!(matches!(
        output.output().export.core_protocols,
        scoop_hir::CoreProtocols::Imported(_)
    ));
    assert!(matches!(
        output.output().local.core_protocols,
        scoop_hir::concrete::ConcreteCoreProtocols::Imported(_)
    ));
    assert!(matches!(
        output.output().local.materialization(),
        scoop_hir::LocalConcreteMaterializationContract::Ordinary
    ));
    assert_eq!(output.imported_core().callable_count(), 0);
    assert_eq!(output.imported_core().type_count(), 0);
    assert_eq!(output.imported_core().value_count(), 0);
}

#[test]
fn ordinary_executable_selects_current_main_under_imported_core_authority() {
    let core = trusted_core();
    let ordinary = parsed_ordinary(file(vec![fun("main", Vec::new())]));
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();

    let output = lower_ordinary_core_only(scoop_identity::RequestedConeKind::Executable, &input)
        .expect("ordinary executable lowering uses imported core authority");

    assert!(matches!(
        output.output().output_kind(),
        scoop_hir::ConeOutputKind::Executable { .. }
    ));
    assert!(
        output
            .output()
            .export
            .source_files
            .iter()
            .all(|source| { source.identity.cone() != ConeIdentity::CORE })
    );
}

#[test]
fn ordinary_calls_select_one_strong_core_binding_and_reuse_its_typed_use() {
    let core = trusted_core_with_answer();
    let ordinary = parsed_ordinary(file(vec![fun(
        "main",
        vec![
            stmt(call("coreAnswer", Vec::new())),
            stmt(call("coreAnswer", Vec::new())),
        ],
    )]));
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &core.strong_callables)
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();

    let output = lower_ordinary_core_only(scoop_identity::RequestedConeKind::Executable, &input)
        .expect("a param-free strong core callable is available to ordinary HIR");

    assert_eq!(output.imported_core().callable_count(), 1);
    assert_eq!(output.output().export.imported_core_callables.len(), 1);
    assert_eq!(output.output().local.imported_core_callables.len(), 1);
    assert_eq!(
        scoop_hir::dump(&output.output().export)
            .matches("ImportedCoreCall #0")
            .count(),
        2
    );
}

#[test]
fn ordinary_call_rejects_a_core_candidate_without_strong_implementation() {
    let core = trusted_core_with_answer();
    let ordinary = parsed_ordinary(file(vec![fun(
        "main",
        vec![stmt(call("coreAnswer", Vec::new()))],
    )]));
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();

    let diagnostics =
        match lower_ordinary_core_only(scoop_identity::RequestedConeKind::Executable, &input) {
            Ok(_) => {
                panic!("a raw HIR candidate cannot stand in for a strong implementation proof")
            }
            Err(diagnostics) => diagnostics,
        };

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("SCOOPC_CAPABILITY_CORE_IMPLEMENTATION_UNAVAILABLE")
    }));
}

#[test]
fn ordinary_call_rejects_a_generic_core_candidate_with_the_stable_capability_code() {
    let core = trusted_core();
    let ordinary = parsed_ordinary(file(vec![fun(
        "main",
        vec![stmt(call("print", vec![int_lit(1)]))],
    )]));
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();

    let diagnostics =
        match lower_ordinary_core_only(scoop_identity::RequestedConeKind::Executable, &input) {
            Ok(_) => panic!("generic core prelude candidates are unavailable in M23-3"),
            Err(diagnostics) => diagnostics,
        };

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("SCOOPC_CAPABILITY_CORE_GENERIC_UNAVAILABLE")
    }));
}

#[test]
fn current_function_shadows_an_imported_core_prelude_callable() {
    let core = trusted_core_with_answer();
    let ordinary = parsed_ordinary(file(vec![
        fun("coreAnswer", Vec::new()),
        fun("main", vec![stmt(call("coreAnswer", Vec::new()))]),
    ]));
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &core.strong_callables)
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();

    let output = lower_ordinary_core_only(scoop_identity::RequestedConeKind::Executable, &input)
        .expect("the current package layer wins before core prelude lookup");

    assert_eq!(output.imported_core().callable_count(), 0);
    assert!(output.output().export.imported_core_callables.is_empty());
}

struct TrustedCoreFixture {
    foundation: scoop_hir::ImportedHirFoundation,
    interface: scoop_hir::CoreHirInterfaceV1,
    strong_callables: Vec<scoop_identity::PersistentExportBindingId>,
    _session: SemanticIdentitySession,
}

fn trusted_core() -> TrustedCoreFixture {
    trusted_core_from_source(complete_core_file(), None)
}

fn trusted_core_with_answer() -> TrustedCoreFixture {
    let mut source = complete_core_file();
    source.declarations.push(fun_expr(
        "coreAnswer",
        Vec::new(),
        Vec::new(),
        Some(ty_named("Int")),
        int_lit(42),
    ));
    make_core_public(&mut source);
    trusted_core_from_source(source, Some("coreAnswer"))
}

fn trusted_core_from_source(
    source: scoop_ast::SourceFile,
    strong_callable: Option<&str>,
) -> TrustedCoreFixture {
    let parsed = parsed_core(source);
    let input = CoreBootstrapSources::try_new(&parsed).unwrap();
    let output = lower_core_bootstrap(&input).unwrap();
    let interface = scoop_hir::CoreHirInterfaceV1::from_core_export(&output.export).unwrap();
    let strong_callables = strong_callable
        .map(|name| {
            let function = output
                .export
                .top_level
                .iter()
                .copied()
                .find(|function| output.export.functions[*function].name == name)
                .unwrap();
            let scoop_hir::HirFunctionIdentity::Source(
                scoop_hir::HirSourceFunctionIdentity::Plain(identity),
            ) = &output.export.function_identities[function]
            else {
                panic!("test strong callable has a plain source identity")
            };
            interface
                .callable_targets()
                .targets()
                .iter()
                .find(|target| {
                    target.definition()
                        == scoop_hir::CoreCallableDefinitionV1::Function(identity.id())
                })
                .unwrap()
                .binding()
        })
        .into_iter()
        .collect::<Vec<_>>();
    let canonical = scoop_hir::CanonicalHirFoundation::from_modules(
        &output.export,
        &output.local,
        &output.native_boundary_types,
    )
    .unwrap();
    let decoded: scoop_hir::DecodedHirFoundation =
        decode_canonical(&encode(&canonical).unwrap(), DecodeLimits::default()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let imported = session
        .import(
            ConeIdentity::CORE,
            SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            &identities,
        )
        .unwrap();
    let (hir, _, _) = imported.into_parts();
    let foundation = scoop_hir::ImportedHirFoundation::from_odr_free(
        scoop_hir::OdrFreeHirFoundation::try_new(canonical).unwrap(),
        hir,
    );
    TrustedCoreFixture {
        foundation,
        interface,
        strong_callables,
        _session: session,
    }
}

fn parsed_core(source: scoop_ast::SourceFile) -> CurrentConeParsedSources {
    let identity = super::core_source_identity("src/core.scoop");
    parsed_sources(identity, source, "<core>")
}

fn parsed_ordinary(source: scoop_ast::SourceFile) -> CurrentConeParsedSources {
    parsed_sources(test_source_identity("src/main.scoop"), source, "<main>")
}

fn parsed_sources(
    identity: SourceIdentity,
    source: scoop_ast::SourceFile,
    display: &str,
) -> CurrentConeParsedSources {
    CurrentConeParsedSources::try_new(
        AllParsedSources::try_new(NonEmptyVec::new(
            IdentifiedParsedSource::new(identity.clone(), source),
            Vec::new(),
        ))
        .unwrap(),
        NonEmptyVec::new(
            CurrentSourceText::new(identity.clone(), String::new()),
            Vec::new(),
        ),
        NonEmptyVec::new(
            CurrentSourceDiagnosticContext::new(identity, PathBuf::from(display)),
            Vec::new(),
        ),
    )
    .unwrap()
}
