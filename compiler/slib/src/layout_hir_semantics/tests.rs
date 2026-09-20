mod artifact;
mod diamond;
mod mir_closure;
mod public_authority;
mod type_authority;

use scoop_hir::{
    SelectedExternalTypeUseV1, SelectedTypeUseV1, SourceNominalId,
    TypeSectionExportValidationError, TypeSectionSemanticValidationError,
    TypeSelectionValidationError,
};
use scoop_identity::{ConeIdentity, CoreBuiltinNominal};

use super::*;
use crate::{
    layout_compile_closure::layout_hir_semantic_closure_for_test,
    strong_compile_decode::tests::cone_named,
};
use artifact::{
    artifact_bytes, artifact_bytes_with_dependencies, checked_artifact,
    checked_artifact_with_authorities, dependency_record, identity_graph, nominal_artifact_bytes,
    selecting_artifact_bytes, target,
};
use public_authority::RecordingPublicFactory;
use type_authority::{
    EmptyCommittedUses, EmptyDeclarations, EmptyDefaults, EmptyFoundation, TestAuthorityError,
};

#[test]
fn complete_empty_semantics_are_lent_from_the_scoped_arena() {
    let bytes = artifact_bytes("hir-complete-empty");
    let provider = checked_artifact(&bytes);
    let identity = provider.identity();
    let mut closure = layout_hir_semantic_closure_for_test(
        ConeIdentity::CORE,
        target(),
        Vec::new(),
        vec![provider],
        vec![vec![]],
    );
    let foundation = EmptyFoundation::new(identity);
    let mut declarations = EmptyDeclarations::new();
    let mut defaults = EmptyDefaults::new(identity);
    let uses = EmptyCommittedUses::default();
    let mut public = RecordingPublicFactory::new(identity);
    let mut authorities = [LayoutHirProviderSemanticAuthoritiesV1::new(
        identity,
        &mut public,
        &foundation,
        &mut declarations,
        &mut defaults,
        &uses,
    )];

    closure
        .with_checked_hir_semantics(&mut authorities, |checked| {
            let providers = checked.dependency_first().collect::<Vec<_>>();
            assert_eq!(providers.len(), 1);
            assert_eq!(providers[0].provider(), identity);
            assert_eq!(providers[0].type_semantics().provider(), identity);
            assert_eq!(checked.provider(identity).unwrap().position(), 0);
        })
        .unwrap();
}

#[test]
fn complete_semantics_uses_independent_source_roots() {
    let bytes = artifact_bytes("hir-missing-root");
    let provider = checked_artifact(&bytes);
    let identity = provider.identity();
    let mut closure = layout_hir_semantic_closure_for_test(
        ConeIdentity::CORE,
        target(),
        Vec::new(),
        vec![provider],
        vec![vec![]],
    );
    let missing = SourceNominalId::Concrete(CoreBuiltinNominal::Unit.identity_record().id());
    let foundation = EmptyFoundation::with_missing_root(identity, missing);
    let mut declarations = EmptyDeclarations::new();
    let mut defaults = EmptyDefaults::new(identity);
    let uses = EmptyCommittedUses::default();
    let mut public = RecordingPublicFactory::new(identity);
    let mut authorities = [LayoutHirProviderSemanticAuthoritiesV1::new(
        identity,
        &mut public,
        &foundation,
        &mut declarations,
        &mut defaults,
        &uses,
    )];

    assert!(matches!(
        closure.with_checked_hir_semantics(&mut authorities, |_| ()),
        Err(CrossConeLayoutHirSemanticClosureError::Types {
            provider,
            source,
        }) if provider == identity && matches!(
            source.as_ref(),
            TypeSectionSemanticValidationError::Exports(exports)
                if matches!(
                    exports.as_ref(),
                    TypeSectionExportValidationError::Source(
                        TestAuthorityError::MissingSourceRoot(actual)
                    ) if *actual == missing
                )
        )
    ));
}

#[test]
fn complete_semantics_rejects_recursive_provenance_before_publication() {
    let (bytes, nominal) = nominal_artifact_bytes("hir-recursive-provenance");
    let provider = checked_artifact(&bytes);
    let identity = provider.identity();
    let mut closure = layout_hir_semantic_closure_for_test(
        ConeIdentity::CORE,
        target(),
        Vec::new(),
        vec![provider],
        vec![vec![]],
    );
    let foundation = EmptyFoundation::with_nominal(&nominal);
    let mut declarations = EmptyDeclarations::with_nominal(&nominal);
    let mut defaults = EmptyDefaults::with_nominal(&nominal);
    let uses = EmptyCommittedUses::with_invalid_recursive_origin(identity, nominal.exact);
    let mut public = RecordingPublicFactory::new(identity);
    let mut authorities = [LayoutHirProviderSemanticAuthoritiesV1::new(
        identity,
        &mut public,
        &foundation,
        &mut declarations,
        &mut defaults,
        &uses,
    )];

    assert!(matches!(
        closure.with_checked_hir_semantics(&mut authorities, |_| ()),
        Err(CrossConeLayoutHirSemanticClosureError::Types { provider, source })
            if provider == identity && matches!(
                source.as_ref(),
                TypeSectionSemanticValidationError::Selected(selected)
                    if matches!(
                        selected.as_ref(),
                        TypeSelectionValidationError::LocalSupportOrigin
                    )
            )
    ));
    assert_eq!(uses.validation_calls(), 1);
}
