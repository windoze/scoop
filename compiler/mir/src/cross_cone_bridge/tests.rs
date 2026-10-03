use scoop_hir::{CanonicalHirFoundation, DecodedHirFoundation};
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity, CoreBuiltinNominal,
    DeclarationScope, DefinitionOwnerChain, DependencyCallableDeclarationId, Effect,
    ExactCallableSignature, ExactTypeKey, PackagePath, PendingIdentityValidation,
    PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
    StrongCallableDefinitionOwner, ValidatedIdentityGraph,
};
use scoop_wire::{decode_canonical, encode};

use super::{
    CrossConeMirBridgeBuildError, CrossConeMirBridgeRelationError, CrossConeMirBridgeSectionV1,
    CrossConeMirBridgeValidationError, DecodedCrossConeMirBridgeSectionV1,
    ParamFreeMirCallableBuildError, ParamFreeMirCallableExportV1, SelectedDependencyMirCallableV1,
    SelectedExternalMirSet, SelectedExternalMirSetBuildError,
};
use crate::{
    CallableSignatureRecord, CallableSignatureSubject, CanonicalMirFoundation,
    DecodedMirFoundation, OdrFreeMirFoundation,
};

#[test]
fn empty_bridge_has_the_fixed_wire_shape() {
    let foundation = OdrFreeMirFoundation::try_new(CanonicalMirFoundation::empty()).unwrap();
    let section = CrossConeMirBridgeSectionV1::try_new(
        ConeIdentity::SINGLE_FILE,
        &foundation,
        Vec::new(),
        Vec::new(),
    )
    .unwrap();

    assert_eq!(
        encode(&section).unwrap(),
        vec![0xa2, 0x01, 0x80, 0x02, 0x80]
    );
}

#[test]
fn bridge_round_trips_typed_exports_and_selected_uses() {
    let fixture = fixture();
    let section = fixture.section();
    let bytes = encode(&section).unwrap();
    let decoded: DecodedCrossConeMirBridgeSectionV1 = decode_canonical(&bytes).unwrap();
    let (mut identities, foundation) = fixture.validated_foundation();

    assert_eq!(
        decoded.validate(fixture.artifact, &mut identities, &foundation),
        Ok(section)
    );
}

#[test]
fn callable_records_reject_mismatched_implementation_and_suspend() {
    let fixture = fixture();
    let declaration = DependencyCallableDeclarationId::Function(fixture.local_function.id());

    assert!(matches!(
        ParamFreeMirCallableExportV1::try_new(
            declaration,
            StrongCallableDefinitionOwner::Function(fixture.foreign_function.id()),
            fixture.signature.clone(),
            crate::GcEffect::Managed,
        ),
        Err(ParamFreeMirCallableBuildError::ImplementationMismatch { .. })
    ));
    assert_eq!(
        ParamFreeMirCallableExportV1::try_new(
            declaration,
            StrongCallableDefinitionOwner::Function(fixture.local_function.id()),
            ExactCallableSignature::new(
                Effect::Suspend,
                None,
                Vec::new(),
                fixture.signature.result(),
            ),
            crate::GcEffect::Managed,
        ),
        Err(ParamFreeMirCallableBuildError::Suspend { declaration })
    );
}

#[test]
fn core_exports_use_the_common_strong_implementation_contract() {
    let fixture = fixture_for(ConeIdentity::CORE);
    let export = fixture.export();
    let section = CrossConeMirBridgeSectionV1::try_new(
        fixture.artifact,
        &fixture.foundation,
        vec![export.clone()],
        Vec::new(),
    )
    .unwrap();
    assert_eq!(section.exports(), &[export]);
}

#[test]
fn bridge_rejects_current_selections() {
    let fixture = fixture();
    assert_eq!(
        CrossConeMirBridgeSectionV1::try_new(
            fixture.artifact,
            &fixture.foundation,
            Vec::new(),
            vec![fixture.selected_from(fixture.artifact)],
        ),
        Err(CrossConeMirBridgeBuildError::Relation(
            CrossConeMirBridgeRelationError::SelectedCurrentProvider {
                index: 0,
                provider: fixture.artifact,
            },
        )),
    );
}

#[test]
fn core_provider_uses_common_callable_selection() {
    let fixture = fixture_with_provider(cone("consumer"), ConeIdentity::CORE);
    let selected = fixture.selected_from(fixture.provider);
    let bridge = CrossConeMirBridgeSectionV1::try_new(
        fixture.artifact,
        &fixture.foundation,
        Vec::new(),
        vec![selected.clone()],
    )
    .unwrap();
    assert_eq!(bridge.selected(), std::slice::from_ref(&selected));
    let selection =
        SelectedExternalMirSet::try_from_callables(fixture.artifact, vec![selected.clone()])
            .unwrap();
    assert_eq!(
        selection
            .dependency_callables()
            .cloned()
            .collect::<Vec<_>>(),
        vec![selected]
    );
}

#[test]
fn bridge_replays_each_export_signature_from_the_mir_foundation() {
    let fixture = fixture();
    let empty = OdrFreeMirFoundation::try_new(CanonicalMirFoundation::empty()).unwrap();
    assert!(matches!(
        CrossConeMirBridgeSectionV1::try_new(
            fixture.artifact,
            &empty,
            vec![fixture.export()],
            Vec::new(),
        ),
        Err(CrossConeMirBridgeBuildError::Relation(
            CrossConeMirBridgeRelationError::MissingExportImplementation { index: 0, .. }
        ))
    ));

    let wrong_signature = ExactCallableSignature::new(
        Effect::Ordinary,
        Some(fixture.signature.result()),
        Vec::new(),
        fixture.signature.result(),
    );
    let wrong = ParamFreeMirCallableExportV1::try_new(
        DependencyCallableDeclarationId::Function(fixture.local_function.id()),
        StrongCallableDefinitionOwner::Function(fixture.local_function.id()),
        wrong_signature,
        crate::GcEffect::Managed,
    )
    .unwrap();
    assert!(matches!(
        CrossConeMirBridgeSectionV1::try_new(
            fixture.artifact,
            &fixture.foundation,
            vec![wrong],
            Vec::new(),
        ),
        Err(CrossConeMirBridgeBuildError::Relation(
            CrossConeMirBridgeRelationError::ExportSignatureMismatch { index: 0, .. }
        ))
    ));
}

#[test]
fn reader_rejects_noncanonical_and_duplicate_tables() {
    let fixture = fixture();
    let first = fixture.selected_from(fixture.provider);
    let second_provider = cone("other-provider");
    let second = fixture.selected_from(second_provider);
    let section = CrossConeMirBridgeSectionV1::try_new(
        fixture.artifact,
        &fixture.foundation,
        Vec::new(),
        vec![first.clone(), second.clone()],
    )
    .unwrap();
    let mut decoded = decode(&section);
    decoded.selected.swap(0, 1);
    let (mut identities, foundation) = fixture.validated_foundation_with(second_provider);
    assert!(matches!(
        decoded.validate(fixture.artifact, &mut identities, &foundation),
        Err(CrossConeMirBridgeValidationError::NonCanonicalSelectedOrder { index: 1 })
    ));

    let duplicate = CrossConeMirBridgeSectionV1 {
        artifact: fixture.artifact,
        exports: Vec::new().into(),
        selected: vec![first.clone(), first].into(),
    };
    let decoded = decode(&duplicate);
    let (mut identities, foundation) = fixture.validated_foundation();
    assert!(matches!(
        decoded.validate(fixture.artifact, &mut identities, &foundation),
        Err(CrossConeMirBridgeValidationError::DuplicateSelected { index: 1, .. })
    ));
}

#[test]
fn reader_rejects_unknown_or_open_section_shapes() {
    for bytes in [
        vec![0xa1, 0x01, 0x80],
        vec![0xa3, 0x01, 0x80, 0x02, 0x80, 0x03, 0x80],
        vec![0xa2, 0x01, 0x80, 0x02, 0x81, 0xa4, 0x01],
    ] {
        assert!(decode_canonical::<DecodedCrossConeMirBridgeSectionV1>(&bytes,).is_err());
    }
}

#[test]
fn producer_side_selection_is_canonical_and_closed() {
    let fixture = fixture();
    let other_provider = cone("other-provider");
    let mut expected = vec![
        fixture.selected_from(fixture.provider),
        fixture.selected_from(other_provider),
    ];
    expected.sort_unstable_by_key(|callable| (callable.provider(), callable.declaration()));

    let selection = SelectedExternalMirSet::try_from_callables(
        fixture.artifact,
        expected.iter().cloned().rev().collect(),
    )
    .unwrap();
    assert_eq!(
        selection
            .dependency_callables()
            .cloned()
            .collect::<Vec<_>>(),
        expected
    );

    let duplicate = fixture.selected_from(fixture.provider);
    assert!(matches!(
        SelectedExternalMirSet::try_from_callables(
            fixture.artifact,
            vec![duplicate.clone(), duplicate],
        ),
        Err(SelectedExternalMirSetBuildError::DuplicateCallable { .. })
    ));

    assert_eq!(
        SelectedExternalMirSet::try_from_callables(
            fixture.artifact,
            vec![fixture.selected_from(fixture.artifact)],
        )
        .err(),
        Some(SelectedExternalMirSetBuildError::SelectedCurrentProvider {
            provider: fixture.artifact,
        }),
    );
}

fn decode(section: &CrossConeMirBridgeSectionV1) -> DecodedCrossConeMirBridgeSectionV1 {
    decode_canonical(&encode(section).unwrap()).unwrap()
}

struct Fixture {
    artifact: ConeIdentity,
    provider: ConeIdentity,
    hir: CanonicalHirFoundation,
    mir: CanonicalMirFoundation,
    foundation: OdrFreeMirFoundation,
    local_function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    foreign_function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    signature: ExactCallableSignature,
}

impl Fixture {
    fn export(&self) -> ParamFreeMirCallableExportV1 {
        ParamFreeMirCallableExportV1::try_new(
            DependencyCallableDeclarationId::Function(self.local_function.id()),
            StrongCallableDefinitionOwner::Function(self.local_function.id()),
            self.signature.clone(),
            crate::GcEffect::Managed,
        )
        .unwrap()
    }

    fn selected_from(&self, provider: ConeIdentity) -> SelectedDependencyMirCallableV1 {
        SelectedDependencyMirCallableV1::try_new(
            provider,
            DependencyCallableDeclarationId::Function(self.foreign_function.id()),
            StrongCallableDefinitionOwner::Function(self.foreign_function.id()),
            self.signature.clone(),
        )
        .unwrap()
    }

    fn section(&self) -> CrossConeMirBridgeSectionV1 {
        CrossConeMirBridgeSectionV1::try_new(
            self.artifact,
            &self.foundation,
            vec![self.export()],
            vec![self.selected_from(self.provider)],
        )
        .unwrap()
    }

    fn validated_foundation(&self) -> (ValidatedIdentityGraph, OdrFreeMirFoundation) {
        self.validated_foundation_with(self.provider)
    }

    fn validated_foundation_with(
        &self,
        extra_provider: ConeIdentity,
    ) -> (ValidatedIdentityGraph, OdrFreeMirFoundation) {
        let hir: DecodedHirFoundation = decode_canonical(&encode(&self.hir).unwrap()).unwrap();
        let mir: DecodedMirFoundation = decode_canonical(&encode(&self.mir).unwrap()).unwrap();
        let mut pending = PendingIdentityValidation::new();
        for authority in [ConeIdentity::CORE, self.artifact, self.provider] {
            pending.register_authority(authority).unwrap();
        }
        if extra_provider != self.provider {
            pending.register_authority(extra_provider).unwrap();
        }
        hir.register_identities(&mut pending).unwrap();
        mir.register_identities(&mut pending).unwrap();
        hir.resolve_identities(&mut pending).unwrap();
        mir.resolve_identities(&mut pending).unwrap();
        let mut identities = pending.finish().unwrap();
        let foundation = mir.validate(&mut identities).unwrap();
        (
            identities,
            OdrFreeMirFoundation::from_validated(foundation).unwrap(),
        )
    }
}

fn fixture() -> Fixture {
    fixture_for(cone("consumer"))
}

fn fixture_for(artifact: ConeIdentity) -> Fixture {
    fixture_with_provider(artifact, cone("provider"))
}

fn fixture_with_provider(artifact: ConeIdentity, provider: ConeIdentity) -> Fixture {
    let local_function = CborIdentityRecord::from_key(source_function(artifact, "local")).unwrap();
    let foreign_function =
        CborIdentityRecord::from_key(source_function(provider, "foreign")).unwrap();
    let unit = CoreBuiltinNominal::Unit.identity_record();
    let exact_unit = CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit.id())).unwrap();
    let signature =
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact_unit.id());
    let mut hir = CanonicalHirFoundation::empty();
    hir.set_types(vec![unit]).unwrap();
    hir.set_functions(vec![local_function.clone(), foreign_function.clone()])
        .unwrap();
    hir.set_exact_types(vec![exact_unit]).unwrap();
    let mut mir = CanonicalMirFoundation::empty();
    mir.set_callable_signatures(vec![CallableSignatureRecord::new(
        CallableSignatureSubject::Strong(
            StrongCallableDefinitionOwner::Function(local_function.id()).callable_owner(),
        ),
        signature.clone(),
    )])
    .unwrap();
    let foundation = OdrFreeMirFoundation::try_new(mir.clone()).unwrap();
    Fixture {
        artifact,
        provider,
        hir,
        mir,
        foundation,
        local_function,
        foreign_function,
        signature,
    }
}

fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("tests", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}

fn source_function(cone: ConeIdentity, name: &str) -> SourceDeclarationKey {
    SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            cone,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    )
}
