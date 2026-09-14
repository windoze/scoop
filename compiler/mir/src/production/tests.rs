use scoop_hir::CanonicalHirFoundation;
use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal,
    DeclarationScope, DefinitionOwnerChain, Effect, ExactCallableSignature,
    ExactOrdinaryNoArgUnitSignature, ExactTypeKey, ExecutableSourceEntryIdentity, ExportBindingKey,
    PackagePath, PendingIdentityValidation, PersistentExactTypeId, PersistentExportBindingId,
    PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite, ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{CallableSignatureRecord, CanonicalMirFoundation, DecodedMirFoundation};

#[test]
fn bridge_section_has_a_fixed_wire_vector_and_validates_against_mir() {
    let fixture = fixture();
    assert_eq!(
        hex(&encode(&fixture.section).unwrap()),
        "a301a2000201a20181a301582020cf0fbdf4e2b62ff6700a8791871842761fde1527a35980126af1567becc47602582030e4b1927a81fbe498d6b5e02580f89a3da5912f818139892760a6c6ea037df103a2000101582030e4b1927a81fbe498d6b5e02580f89a3da5912f818139892760a6c6ea037df10281a2015820ea1de3597e0f30acca2c62c6d871e693648ef7e1097cc8b0082d316acd7e76390258201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc02a100010381a201a2000101582030e4b1927a81fbe498d6b5e02580f89a3da5912f818139892760a6c6ea037df102a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc"
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
        vec![0xa2],
        vec![0xa4],
        vec![
            0xa3, 0x01, 0xa1, 0x00, 0x01, 0x02, 0xa1, 0x00, 0x01, 0x04, 0x80,
        ],
        vec![
            0xa3, 0x01, 0xa1, 0x00, 0x03, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80,
        ],
        vec![
            0xa3, 0x01, 0xa2, 0x00, 0x01, 0x01, 0x00, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80,
        ],
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
fn validation_requires_total_strong_signature_coverage() {
    let fixture = fixture();
    let invalid = CoreBootstrapBridgeSectionV1 {
        core_bridge: fixture.section.core_bridge.clone(),
        entry_bridge: fixture.section.entry_bridge.clone(),
        strong_callable_bridges: StrongCallableBridgeSurfaceV1::try_new(Vec::new()).unwrap(),
    };
    let (mut identities, foundation) = validate_foundations(&fixture);

    assert_eq!(
        decode(&invalid).validate(ConeIdentity::CORE, &mut identities, &foundation),
        Err(MirProductionValidationError::StrongCallableCoverage {
            expected: 1,
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
        core_bridge: fixture.section.core_bridge.clone(),
        entry_bridge: fixture.section.entry_bridge.clone(),
        strong_callable_bridges: StrongCallableBridgeSurfaceV1::try_new(vec![
            StrongCallableBridgeV1::new(
                CallableOwner::Function(fixture.function.id()),
                wrong_signature,
            ),
        ])
        .unwrap(),
    };
    let (mut identities, foundation) = validate_foundations(&fixture);

    assert_eq!(
        decode(&invalid).validate(ConeIdentity::CORE, &mut identities, &foundation),
        Err(MirProductionValidationError::StrongCallableMismatch { index: 0 })
    );
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
        fixture.section.core_bridge.clone(),
        EntryMirBridgeBranchV1::Library,
        strong_callable_bridges,
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
fn builder_closes_core_entry_and_implementation_branches() {
    let fixture = fixture();
    let entry = CborIdentityRecord::from_key(source_function("main")).unwrap();
    assert!(matches!(
        CoreBootstrapBridgeSectionV1::try_new(
            ConeIdentity::CORE,
            CoreMirBridgeBranchV1::NotCore,
            EntryMirBridgeBranchV1::Library,
            fixture.section.strong_callable_bridges.clone(),
        ),
        Err(MirProductionBuildError::MissingCoreBridge)
    ));
    assert!(matches!(
        CoreBootstrapBridgeSectionV1::try_new(
            ConeIdentity::SINGLE_FILE,
            fixture.section.core_bridge.clone(),
            EntryMirBridgeBranchV1::Library,
            fixture.section.strong_callable_bridges.clone(),
        ),
        Err(MirProductionBuildError::UnexpectedCoreBridge(
            ConeIdentity::SINGLE_FILE
        ))
    ));
    assert!(matches!(
        CoreBootstrapBridgeSectionV1::try_new(
            ConeIdentity::CORE,
            fixture.section.core_bridge.clone(),
            EntryMirBridgeBranchV1::Executable(Box::new(
                EntryMirBridgeV1::new(
                    entry_source(&entry, fixture.exact_unit),
                    CallableOwner::Function(entry.id()),
                )
                .unwrap(),
            )),
            fixture.section.strong_callable_bridges.clone(),
        ),
        Err(MirProductionBuildError::CoreMustBeLibrary)
    ));
    assert!(matches!(
        CoreMirCallableBridgeV1::new(
            fixture.binding.id(),
            fixture.function.id(),
            CallableOwner::Function(other_function_id()),
        ),
        Err(MirProductionBuildError::CoreImplementationMismatch { .. })
    ));
    assert!(matches!(
        CoreBootstrapBridgeSectionV1::try_new(
            ConeIdentity::SINGLE_FILE,
            CoreMirBridgeBranchV1::NotCore,
            EntryMirBridgeBranchV1::Executable(Box::new(
                EntryMirBridgeV1::new(
                    entry_source(&entry, fixture.exact_unit),
                    CallableOwner::Function(entry.id()),
                )
                .unwrap(),
            )),
            fixture.section.strong_callable_bridges,
        ),
        Err(MirProductionBuildError::ForeignEntrySource {
            artifact: ConeIdentity::SINGLE_FILE,
            entry: ConeIdentity::CORE,
        })
    ));
}

#[test]
fn core_shape_roots_require_one_exact_source_nominal_identity() {
    let source = CoreBuiltinNominal::Unit.identity_record().id();
    let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source)).unwrap();
    let root = CoreMirShapeSupportRootV1::new(source, exact).unwrap();
    assert_eq!(root.source(), source);
    assert_eq!(root.exact(), exact);

    let wrong = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Any.identity_record().id(),
    ))
    .unwrap();
    assert!(matches!(
        CoreMirShapeSupportRootV1::new(source, wrong),
        Err(MirProductionBuildError::CoreShapeExactMismatch {
            source: actual_source,
            expected,
            actual,
        }) if actual_source == source && expected == exact && actual == wrong
    ));
    assert_eq!(
        CoreMirBridgeV1::try_new(Vec::new(), vec![root, root]),
        Err(MirProductionBuildError::DuplicateCoreShapeSource(source))
    );
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
        CoreMirBridgeBranchV1::NotCore,
        EntryMirBridgeBranchV1::Executable(Box::new(
            EntryMirBridgeV1::new(source.clone(), CallableOwner::Function(function.id())).unwrap(),
        )),
        bridges,
    )
    .unwrap();
    let bytes = encode(&section).unwrap();
    assert_eq!(
        hex(&bytes),
        "a301a1000102a2000201a201a501582000769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b60258209714f93e5ddfe8681524ef2048f90a96e0e87dcbc813b55879c836164da02cdd03a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc0458205a43bee43f27e5c33d012c1129702324d18dd3856d3158b657383cc2d61c9257055820231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde02a200010158209714f93e5ddfe8681524ef2048f90a96e0e87dcbc813b55879c836164da02cdd0381a201a200010158209714f93e5ddfe8681524ef2048f90a96e0e87dcbc813b55879c836164da02cdd02a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc"
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

fn other_function_id() -> PersistentFunctionId {
    CborIdentityRecord::from_key(source_function("other"))
        .unwrap()
        .id()
}

fn source_function(name: &str) -> SourceDeclarationKey {
    source_function_in(ConeIdentity::CORE, name)
}

fn source_function_in(cone: ConeIdentity, name: &str) -> SourceDeclarationKey {
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

fn entry_source(
    declaration: &CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    exact_unit: PersistentExactTypeId,
) -> ExecutableSourceEntryIdentity {
    ExecutableSourceEntryIdentity::try_new(
        declaration,
        ExactOrdinaryNoArgUnitSignature::new(exact_unit),
    )
    .unwrap()
}

fn decode(section: &CoreBootstrapBridgeSectionV1) -> DecodedCoreBootstrapBridgeSectionV1 {
    decode_canonical(&encode(section).unwrap(), DecodeLimits::default()).unwrap()
}

struct Fixture {
    hir: CanonicalHirFoundation,
    mir: CanonicalMirFoundation,
    section: CoreBootstrapBridgeSectionV1,
    function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    other_function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    binding: CborIdentityRecord<PersistentExportBindingId, ExportBindingKey>,
    exact_unit: PersistentExactTypeId,
}

fn fixture() -> Fixture {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("printLine").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = CborIdentityRecord::from_key(declaration.clone()).unwrap();
    let other_function = CborIdentityRecord::from_key(source_function("other")).unwrap();
    let binding = CborIdentityRecord::from_key(ExportBindingKey::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        CanonicalIdentifier::new("printLine").unwrap(),
        BindingTarget::function(&declaration).unwrap(),
    ))
    .unwrap();
    let unit_record = CoreBuiltinNominal::Unit.identity_record();
    let unit_type = unit_record.id();
    let exact_unit_record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit_type)).unwrap();
    let exact_unit = exact_unit_record.id();
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact_unit);

    let mut hir = CanonicalHirFoundation::empty();
    hir.set_types(vec![unit_record]).unwrap();
    hir.set_functions(vec![function.clone(), other_function.clone()])
        .unwrap();
    hir.set_exact_types(vec![exact_unit_record]).unwrap();
    hir.set_export_bindings(vec![binding.clone()]).unwrap();

    let mut mir = CanonicalMirFoundation::empty();
    mir.set_callable_signatures(vec![CallableSignatureRecord::new(
        CallableSignatureSubject::Strong(CallableOwner::Function(function.id())),
        signature.clone(),
    )])
    .unwrap();
    let strong_callable_bridges = StrongCallableBridgeSurfaceV1::from_odr_free_foundation(
        &OdrFreeMirFoundation::try_new(mir.clone()).unwrap(),
    );
    let core_bridge = CoreMirBridgeV1::try_new(
        vec![
            CoreMirCallableBridgeV1::new(
                binding.id(),
                function.id(),
                CallableOwner::Function(function.id()),
            )
            .unwrap(),
        ],
        vec![CoreMirShapeSupportRootV1::new(unit_type, exact_unit).unwrap()],
    )
    .unwrap();
    let section = CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        CoreMirBridgeBranchV1::Core(core_bridge),
        EntryMirBridgeBranchV1::Library,
        strong_callable_bridges,
    )
    .unwrap();

    Fixture {
        hir,
        mir,
        section,
        function,
        other_function,
        binding,
        exact_unit,
    }
}

fn validate_foundations(fixture: &Fixture) -> (ValidatedIdentityGraph, ValidatedMirFoundation) {
    validate_mir(&fixture.hir, &fixture.mir)
}

fn validate_mir(
    hir: &CanonicalHirFoundation,
    mir: &CanonicalMirFoundation,
) -> (ValidatedIdentityGraph, ValidatedMirFoundation) {
    let hir: scoop_hir::DecodedHirFoundation =
        decode_canonical(&encode(hir).unwrap(), DecodeLimits::default()).unwrap();
    let mir: DecodedMirFoundation =
        decode_canonical(&encode(mir).unwrap(), DecodeLimits::default()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending
        .register_authority(ConeIdentity::SINGLE_FILE)
        .unwrap();
    hir.register_identities(&mut pending).unwrap();
    mir.register_identities(&mut pending).unwrap();
    hir.resolve_identities(&mut pending).unwrap();
    mir.resolve_identities(&mut pending).unwrap();
    let mut identities = pending.finish().unwrap();
    let foundation = mir
        .validate(
            &mut identities,
            &mut BudgetMeter::new(DecodeLimits::default()),
        )
        .unwrap();
    (identities, foundation)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
