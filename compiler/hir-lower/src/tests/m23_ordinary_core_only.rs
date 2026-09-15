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
fn ordinary_selected_core_call_lowers_to_one_branded_direct_mir_target() {
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
    let hir = lower_ordinary_core_only(scoop_identity::RequestedConeKind::Executable, &input)
        .expect("ordinary HIR selects the trusted-core callable");
    let selected_mir = core.project_selected_callables_to_mir(hir.imported_core());

    let mir = scoop_mir_lower::lower_ordinary(&hir, selected_mir)
        .expect("ordinary MIR retains the exact trusted-core selection");

    assert_eq!(mir.imported_core().len(), 1);
    assert_eq!(mir.module().meta.imported_core_callables.len(), 1);
    let calls =
        mir.module()
            .functions
            .iter()
            .flat_map(|(_, function)| function.body.blocks.iter())
            .flat_map(|(_, block)| &block.statements)
            .filter_map(|statement| {
                let scoop_mir::StatementKind::Call(effect) = &statement.kind else {
                    return None;
                };
                let call = match effect {
                    scoop_mir::CallEffect::Unit(call)
                    | scoop_mir::CallEffect::Value { call, .. } => call,
                };
                matches!(call.target.callee, scoop_mir::Callee::CoreExternal(_)).then_some(call)
            })
            .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert!(
        calls
            .iter()
            .all(|call| matches!(call.target.kind, scoop_mir::CallKind::Direct))
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
    mir_foundation: scoop_mir::ImportedMirFoundation,
    mir_production: scoop_mir::CoreBootstrapBridgeSectionV1,
    strong_callables: Vec<scoop_identity::PersistentExportBindingId>,
    _session: SemanticIdentitySession,
}

impl TrustedCoreFixture {
    fn project_selected_callables_to_mir<'a>(
        &'a self,
        selected: &scoop_hir::SelectedImportedCoreSet<'_>,
    ) -> scoop_mir::SelectedImportedMirSet<'a> {
        let mut projected =
            scoop_mir::SelectedImportedMirSet::new(&self.mir_foundation, &self.mir_production);
        for selected in selected.callable_selections() {
            let scoop_hir::ImportedCorePreludeTarget::Callable(target) = selected.target() else {
                panic!("test selection contains only callables")
            };
            let scoop_hir::CoreCallableDefinitionV1::Function(definition) = target.definition()
            else {
                panic!("test core callable has a source function definition")
            };
            let scoop_hir::CoreHirCallableCapabilityV1::ParamFreeCandidate(signature) =
                target.capability()
            else {
                panic!("test core callable has a param-free exact signature")
            };
            let callable = self
                .mir_foundation
                .project_core_callable(
                    &self.mir_production,
                    selected.binding().persistent(),
                    definition,
                    signature.clone(),
                )
                .unwrap();
            projected.insert(callable).unwrap();
        }
        projected
    }
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
    let strong_targets = strong_callable
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
                .clone()
        })
        .into_iter()
        .collect::<Vec<_>>();
    let strong_callables = strong_targets
        .iter()
        .map(|target| target.binding())
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
    let (hir, mir, _) = imported.into_parts();
    let foundation = scoop_hir::ImportedHirFoundation::from_odr_free(
        scoop_hir::OdrFreeHirFoundation::try_new(canonical).unwrap(),
        hir,
    );
    let strong_mir = strong_targets
        .iter()
        .map(|target| {
            let scoop_hir::CoreCallableDefinitionV1::Function(definition) = target.definition()
            else {
                panic!("test strong target is a function")
            };
            let scoop_hir::CoreHirCallableCapabilityV1::ParamFreeCandidate(signature) =
                target.capability()
            else {
                panic!("test strong target is param-free")
            };
            (target.binding(), definition, signature.clone())
        })
        .collect::<Vec<_>>();
    let mut mir_canonical = scoop_mir::CanonicalMirFoundation::empty();
    mir_canonical
        .set_callable_signatures(
            strong_mir
                .iter()
                .map(|(_, definition, signature)| {
                    scoop_mir::CallableSignatureRecord::new(
                        scoop_mir::CallableSignatureSubject::Strong(
                            scoop_identity::CallableOwner::Function(*definition),
                        ),
                        signature.clone(),
                    )
                })
                .collect(),
        )
        .unwrap();
    let mir_foundation = scoop_mir::ImportedMirFoundation::from_odr_free(
        scoop_mir::OdrFreeMirFoundation::try_new(mir_canonical).unwrap(),
        mir,
    );
    let mir_production = scoop_mir::CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        scoop_mir::CoreMirBridgeBranchV1::Core(
            scoop_mir::CoreMirBridgeV1::try_new(
                strong_mir
                    .iter()
                    .map(|(binding, definition, _)| {
                        scoop_mir::CoreMirCallableBridgeV1::new(
                            *binding,
                            *definition,
                            scoop_identity::CallableOwner::Function(*definition),
                        )
                        .unwrap()
                    })
                    .collect(),
                Vec::new(),
            )
            .unwrap(),
        ),
        scoop_mir::EntryMirBridgeBranchV1::Library,
        scoop_mir::StrongCallableBridgeSurfaceV1::try_new(
            strong_mir
                .iter()
                .map(|(_, definition, signature)| {
                    scoop_mir::StrongCallableBridgeV1::new(
                        scoop_identity::CallableOwner::Function(*definition),
                        signature.clone(),
                    )
                })
                .collect(),
        )
        .unwrap(),
    )
    .unwrap();
    TrustedCoreFixture {
        foundation,
        interface,
        mir_foundation,
        mir_production,
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
