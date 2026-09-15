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

use super::{complete_core_file, file, fun, test_source_identity};
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

struct TrustedCoreFixture {
    foundation: scoop_hir::ImportedHirFoundation,
    interface: scoop_hir::CoreHirInterfaceV1,
    _session: SemanticIdentitySession,
}

fn trusted_core() -> TrustedCoreFixture {
    let parsed = parsed_core();
    let input = CoreBootstrapSources::try_new(&parsed).unwrap();
    let output = lower_core_bootstrap(&input).unwrap();
    let interface = scoop_hir::CoreHirInterfaceV1::from_core_export(&output.export).unwrap();
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
        _session: session,
    }
}

fn parsed_core() -> CurrentConeParsedSources {
    let identity = super::core_source_identity("src/core.scoop");
    parsed_sources(identity, complete_core_file(), "<core>")
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
