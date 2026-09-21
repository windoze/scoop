use scoop_hir::CanonicalHirFoundation;
use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal,
    DeclarationScope, DefinitionOwnerChain, Effect, ExactCallableSignature,
    ExactOrdinaryNoArgUnitSignature, ExactTypeKey, ExecutableSourceEntryIdentity, ExportBindingKey,
    GeneratedCallableKey, LexicalCallableParent, LexicalCallableRole, PackagePath,
    PendingIdentityValidation, PersistentExactTypeId, PersistentFunctionId, SourceDeclarationKey,
    SourceDeclarationSite, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment, ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{CallableSignatureRecord, CanonicalMirFoundation, DecodedMirFoundation};

#[test]
fn bridge_section_has_a_fixed_wire_vector_and_validates_against_mir() {
    let fixture = fixture();
    assert_eq!(
        hex(&encode(&fixture.section).unwrap()),
        "a202a100010382a301a2000101582030e4b1927a81fbe498d6b5e02580f89a3da5912f818139892760a6c6ea037df102a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc0301a301a2000101582098e824248002f28e6d3b39a8b68c54c2baaf96b7afae9ba5812f2f22f4b2ddb502a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc0302"
    );

    let (mut identities, foundation) = validate_foundations(&fixture);
    assert_eq!(
        decode(&fixture.section).validate(ConeIdentity::CORE, &mut identities, &foundation),
        Ok(fixture.section)
    );
}

#[test]
fn bridge_reader_rejects_non_closed_products_and_sums() {
    for bytes in [
        vec![0xa1],
        vec![0xa3],
        vec![0xa2, 0x01, 0xa1, 0x00, 0x01, 0x03, 0x80],
        vec![0xa2, 0x02, 0xa1, 0x00, 0x03, 0x03, 0x80],
        vec![0xa2, 0x02, 0xa2, 0x00, 0x01, 0x01, 0x00, 0x03, 0x80],
    ] {
        assert!(
            decode_canonical::<DecodedCoreBootstrapBridgeSectionV1>(
                &bytes,
                DecodeLimits::default(),
            )
            .is_err()
        );
    }
}

#[test]
fn reader_rejects_removed_core_production_field() {
    for old_branch in [vec![0xa1, 0x00, 0x01], vec![0xa2, 0x00, 0x02, 0x01, 0xa0]] {
        let mut bytes = encode(&fixture().section).unwrap();
        assert_eq!(bytes[0], 0xa2);
        bytes[0] = 0xa3;
        bytes.splice(1..1, std::iter::once(0x01).chain(old_branch));
        let error = decode_canonical::<DecodedCoreBootstrapBridgeSectionV1>(
            &bytes,
            DecodeLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::InvalidLength {
                expected: 2,
                actual: 3
            }
        );
    }
}

#[test]
fn validation_requires_total_strong_signature_coverage() {
    let fixture = fixture();
    let invalid = CoreBootstrapBridgeSectionV1 {
        entry_bridge: fixture.section.entry_bridge.clone(),
        strong_callable_bridges: StrongCallableBridgeSurfaceV1::try_new(Vec::new()).unwrap(),
    };
    let (mut identities, foundation) = validate_foundations(&fixture);

    assert_eq!(
        decode(&invalid).validate(ConeIdentity::CORE, &mut identities, &foundation),
        Err(MirProductionValidationError::StrongCallableCoverage {
            expected: 2,
            actual: 0,
        })
    );
}

#[test]
fn validation_replays_the_foundation_signature() {
    let fixture = fixture();
    let wrong_signature =
        ExactCallableSignature::new(Effect::Suspend, None, Vec::new(), fixture.exact_unit);
    let invalid = CoreBootstrapBridgeSectionV1 {
        entry_bridge: fixture.section.entry_bridge.clone(),
        strong_callable_bridges: StrongCallableBridgeSurfaceV1::try_new(vec![
            StrongCallableBridgeV1::new(
                CallableOwner::Function(fixture.function.id()),
                wrong_signature,
            ),
            StrongCallableBridgeV1::new(
                CallableOwner::Function(fixture.other_function.id()),
                ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), fixture.exact_unit),
            ),
        ])
        .unwrap(),
    };
    let (mut identities, foundation) = validate_foundations(&fixture);

    assert!(matches!(
        decode(&invalid).validate(ConeIdentity::CORE, &mut identities, &foundation),
        Err(MirProductionValidationError::StrongCallableMismatch { .. })
    ));
}

#[test]
fn validation_rejects_noncanonical_strong_callable_order() {
    let fixture = fixture();
    let signature =
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), fixture.exact_unit);
    let mut mir = CanonicalMirFoundation::empty();
    mir.set_callable_signatures(vec![
        CallableSignatureRecord::new(
            CallableSignatureSubject::Strong(CallableOwner::Function(fixture.function.id())),
            signature.clone(),
        ),
        CallableSignatureRecord::new(
            CallableSignatureSubject::Strong(CallableOwner::Function(fixture.other_function.id())),
            signature,
        ),
    ])
    .unwrap();
    let strong_callable_bridges = StrongCallableBridgeSurfaceV1::from_odr_free_foundation(
        &OdrFreeMirFoundation::try_new(mir.clone()).unwrap(),
    );
    let section = CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        EntryMirBridgeBranchV1::Library,
        strong_callable_bridges
            .with_initialization_cycle(fixture.other_function.id())
            .unwrap(),
    )
    .unwrap();
    let mut decoded = decode(&section);
    decoded.strong_callable_bridges.bridges.swap(0, 1);
    let (mut identities, foundation) = validate_mir(&fixture.hir, &mir);

    assert_eq!(
        decoded.validate(ConeIdentity::CORE, &mut identities, &foundation),
        Err(MirProductionValidationError::NonCanonicalStrongCallableOrder { index: 1 })
    );
}

#[test]
fn validation_joins_mixed_callable_owner_variants_by_subject() {
    let fixture = fixture();
    let generated = generated_callable_sorting_before(fixture.function.id());
    let signature =
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), fixture.exact_unit);
    let mut mir = CanonicalMirFoundation::empty();
    mir.set_generated_callables(vec![generated.clone()])
        .unwrap();
    mir.set_callable_signatures(vec![
        CallableSignatureRecord::new(
            CallableSignatureSubject::Strong(CallableOwner::Function(fixture.function.id())),
            signature.clone(),
        ),
        CallableSignatureRecord::new(
            CallableSignatureSubject::Strong(CallableOwner::Generated(generated.id())),
            signature.clone(),
        ),
        CallableSignatureRecord::new(
            CallableSignatureSubject::Strong(CallableOwner::Function(fixture.other_function.id())),
            signature,
        ),
    ])
    .unwrap();
    assert!(matches!(
        mir.callable_signatures()[0].subject(),
        CallableSignatureSubject::Strong(CallableOwner::Generated(_))
    ));

    let strong_callable_bridges = StrongCallableBridgeSurfaceV1::from_odr_free_foundation(
        &OdrFreeMirFoundation::try_new(mir.clone()).unwrap(),
    );
    assert!(matches!(
        strong_callable_bridges.bridges()[0].implementation(),
        CallableOwner::Function(_)
    ));
    let section = CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        EntryMirBridgeBranchV1::Library,
        strong_callable_bridges
            .with_initialization_cycle(fixture.other_function.id())
            .unwrap(),
    )
    .unwrap();
    let (mut identities, foundation) = validate_mir(&fixture.hir, &mir);

    assert_eq!(
        decode(&section).validate(ConeIdentity::CORE, &mut identities, &foundation),
        Ok(section)
    );
}

#[test]
fn builder_closes_callable_roles_and_entry_implementations() {
    let fixture = fixture();
    let entry = CborIdentityRecord::from_key(source_function("main")).unwrap();
    let unmarked = StrongCallableBridgeSurfaceV1::from_odr_free_foundation(
        &OdrFreeMirFoundation::try_new(fixture.mir.clone()).unwrap(),
    );
    assert_eq!(
        CoreBootstrapBridgeSectionV1::try_new(
            ConeIdentity::CORE,
            EntryMirBridgeBranchV1::Library,
            unmarked.clone()
        ),
        Err(MirProductionBuildError::MissingInitializationCycle)
    );
    assert_eq!(
        CoreBootstrapBridgeSectionV1::try_new(
            ConeIdentity::SINGLE_FILE,
            EntryMirBridgeBranchV1::Library,
            fixture.section.strong_callable_bridges.clone()
        ),
        Err(MirProductionBuildError::UnexpectedInitializationCycle(
            ConeIdentity::SINGLE_FILE
        ))
    );
    assert_eq!(
        unmarked
            .clone()
            .with_initialization_cycle(other_function_id()),
        Err(MirProductionBuildError::MissingStrongInitializationCycle(
            CallableOwner::Function(other_function_id())
        ))
    );
    assert_eq!(
        fixture
            .section
            .strong_callable_bridges
            .clone()
            .with_initialization_cycle(fixture.function.id()),
        Err(MirProductionBuildError::DuplicateInitializationCycle)
    );
    assert!(matches!(
        CoreBootstrapBridgeSectionV1::try_new(
            ConeIdentity::CORE,
            EntryMirBridgeBranchV1::Executable(Box::new(
                EntryMirBridgeV1::new(
                    entry_source(&entry, fixture.exact_unit),
                    CallableOwner::Function(entry.id())
                )
                .unwrap()
            )),
            fixture.section.strong_callable_bridges.clone()
        ),
        Err(MirProductionBuildError::CoreMustBeLibrary)
    ));
    assert!(matches!(
        CoreBootstrapBridgeSectionV1::try_new(
            ConeIdentity::SINGLE_FILE,
            EntryMirBridgeBranchV1::Executable(Box::new(
                EntryMirBridgeV1::new(
                    entry_source(&entry, fixture.exact_unit),
                    CallableOwner::Function(entry.id())
                )
                .unwrap()
            )),
            unmarked
        ),
        Err(MirProductionBuildError::ForeignEntrySource {
            artifact: ConeIdentity::SINGLE_FILE,
            entry: ConeIdentity::CORE
        })
    ));
}

#[test]
fn executable_entry_bridge_round_trips_the_complete_hir_proof() {
    let artifact = ConeIdentity::SINGLE_FILE;
    let function = CborIdentityRecord::from_key(source_function_in(artifact, "main")).unwrap();
    let unit_record = CoreBuiltinNominal::Unit.identity_record();
    let unit_type = unit_record.id();
    let exact_unit_record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit_type)).unwrap();
    let source = entry_source(&function, exact_unit_record.id());
    let signature =
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact_unit_record.id());
    let mut hir = CanonicalHirFoundation::empty();
    hir.set_types(vec![unit_record]).unwrap();
    hir.set_functions(vec![function.clone()]).unwrap();
    hir.set_exact_types(vec![exact_unit_record]).unwrap();
    let mut mir = CanonicalMirFoundation::empty();
    mir.set_callable_signatures(vec![CallableSignatureRecord::new(
        CallableSignatureSubject::Strong(CallableOwner::Function(function.id())),
        signature,
    )])
    .unwrap();
    let bridges = StrongCallableBridgeSurfaceV1::from_odr_free_foundation(
        &OdrFreeMirFoundation::try_new(mir.clone()).unwrap(),
    );
    let section = CoreBootstrapBridgeSectionV1::try_new(
        artifact,
        EntryMirBridgeBranchV1::Executable(Box::new(
            EntryMirBridgeV1::new(source.clone(), CallableOwner::Function(function.id())).unwrap(),
        )),
        bridges,
    )
    .unwrap();
    let bytes = encode(&section).unwrap();
    assert_eq!(
        hex(&bytes),
        "a202a2000201a201a501582000769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b60258209714f93e5ddfe8681524ef2048f90a96e0e87dcbc813b55879c836164da02cdd03a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc0458205a43bee43f27e5c33d012c1129702324d18dd3856d3158b657383cc2d61c9257055820231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde02a200010158209714f93e5ddfe8681524ef2048f90a96e0e87dcbc813b55879c836164da02cdd0381a301a200010158209714f93e5ddfe8681524ef2048f90a96e0e87dcbc813b55879c836164da02cdd02a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc0301"
    );

    let (mut identities, foundation) = validate_mir(&hir, &mir);
    let validated = decode(&section)
        .validate(artifact, &mut identities, &foundation)
        .unwrap();
    let EntryMirBridgeBranchV1::Executable(entry) = validated.entry_bridge() else {
        panic!("the executable bridge remains executable")
    };
    assert_eq!(entry.source(), &source);

    let mut tampered = bytes;
    let body = source.main().body();
    let offset = tampered
        .windows(body.as_array().len())
        .position(|window| window == body.as_array())
        .unwrap();
    tampered[offset + body.as_array().len() - 1] ^= 1;
    let decoded: DecodedCoreBootstrapBridgeSectionV1 =
        decode_canonical(&tampered, DecodeLimits::default()).unwrap();
    let (mut identities, foundation) = validate_mir(&hir, &mir);
    assert_eq!(
        decoded.validate(artifact, &mut identities, &foundation),
        Err(MirProductionValidationError::EntrySourceMismatch)
    );
}

#[test]
fn entry_bridge_rejects_the_removed_id_only_wire() {
    let function =
        CborIdentityRecord::from_key(source_function_in(ConeIdentity::SINGLE_FILE, "main"))
            .unwrap();
    let bytes = encode(&IdOnlyEntryBranch(function.id())).unwrap();
    assert!(
        decode_canonical::<DecodedEntryMirBridgeBranchV1>(&bytes, DecodeLimits::default()).is_err()
    );
}

struct IdOnlyEntryBranch(PersistentFunctionId);

impl scoop_wire::WireEncode for IdOnlyEntryBranch {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(2)?;
        encoder.field(1)?;
        encoder.map(2)?;
        encoder.field(1)?;
        self.0.encode(encoder)?;
        encoder.field(2)?;
        CallableOwner::Function(self.0).encode(encoder)
    }
}

mod support;
use support::*;
mod roles;
