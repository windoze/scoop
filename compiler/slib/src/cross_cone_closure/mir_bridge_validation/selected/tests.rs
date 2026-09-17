use scoop_hir::{
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalDependencyBindingWitnessesV1, CanonicalExportConstValuesV1,
    CanonicalExportDefaultTemplatesV1, CanonicalExportDefinitionSourcesV1,
    CanonicalExternalHirReferenceRolesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1,
    CanonicalTypeAliasInterfacesV1, CrossConeHirInterfaceSectionV1, ExternalHirReferenceRoleV1,
    ExternalHirReferenceV1,
};
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, DeclarationScope,
    DefinitionOwnerChain, DependencyCallableDeclarationId, Effect, ExactCallableSignature,
    ExactTypeKey, PackagePath, PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
    StrongCallableDefinitionOwner,
};
use scoop_mir::{
    CanonicalMirFoundation, CrossConeMirBridgeSectionV1, OdrFreeMirFoundation,
    SelectedDependencyMirCallableV1,
};

use super::{CrossConeMirClosureRelationError, dependency_hir_target, validate_hir_selected_set};

#[test]
fn mir_selection_requires_an_external_hir_reference() {
    let fixture = fixture();
    assert_eq!(
        validate_hir_selected_set(&empty_interface(Vec::new()), &fixture.bridge),
        Err(CrossConeMirClosureRelationError::MissingHirSelection {
            declaration: fixture.declaration,
        })
    );
}

#[test]
fn mir_selection_requires_the_concrete_selected_role() {
    let fixture = fixture();
    let reference = ExternalHirReferenceV1::try_new(
        fixture.provider,
        dependency_hir_target(fixture.declaration),
        CanonicalExternalHirReferenceRolesV1::try_new(vec![
            ExternalHirReferenceRoleV1::SignatureDependency,
        ])
        .unwrap(),
        CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        validate_hir_selected_set(&empty_interface(vec![reference]), &fixture.bridge),
        Err(CrossConeMirClosureRelationError::MissingHirSelectionRole {
            declaration: fixture.declaration,
        })
    );
}

struct Fixture {
    provider: scoop_identity::ConeIdentity,
    declaration: DependencyCallableDeclarationId,
    bridge: CrossConeMirBridgeSectionV1,
}

fn fixture() -> Fixture {
    let consumer = cone("consumer");
    let provider = cone("provider");
    let function =
        CborIdentityRecord::<PersistentFunctionId, _>::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                provider,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("run").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap();
    let declaration = DependencyCallableDeclarationId::Function(function.id());
    let exact = scoop_identity::PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ))
    .unwrap();
    let selected = SelectedDependencyMirCallableV1::try_new(
        provider,
        declaration,
        StrongCallableDefinitionOwner::Function(function.id()),
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact),
    )
    .unwrap();
    let foundation = OdrFreeMirFoundation::try_new(CanonicalMirFoundation::empty()).unwrap();
    let bridge =
        CrossConeMirBridgeSectionV1::try_new(consumer, &foundation, Vec::new(), vec![selected])
            .unwrap();
    Fixture {
        provider,
        declaration,
        bridge,
    }
}

fn empty_interface(references: Vec<ExternalHirReferenceV1>) -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(references).unwrap(),
    )
}

fn cone(name: &str) -> scoop_identity::ConeIdentity {
    ConeCoordinate::new("tests", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}
