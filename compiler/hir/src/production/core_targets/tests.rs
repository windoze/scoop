use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionOrigin,
    DefinitionOriginRecord, DefinitionOwnerChain, ExportBindingKey, NormalizedSourcePath,
    PackagePath, SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity,
    SourceNominalKind, SourceSpan,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;

#[test]
fn callable_surface_has_fixed_wire_for_every_capability_variant() {
    let fixture = fixture();
    let bytes = encode(&fixture.surface).unwrap();
    assert_eq!(
        hex(&bytes),
        "83a40158202276f741811791444dedf91f23200f8b25cd4643f1a30e29ebc792ad444f5ea402a20001015820a24c2bda9216ca996a774ff2906edb760420730f30fff1c4c7187a21b38b32fd03a4010102a100010381a200010158201f735aeecea8726a3e13676d04618097f05e2b7bf0f3e428876aa2e014e0b2b504a200010158201f735aeecea8726a3e13676d04618097f05e2b7bf0f3e428876aa2e014e0b2b504a2000101a4010102a1000103815820ba3a2277b6c2dc18604fd5b344ae9d9403abcbe3d55829f2a0d07cddd2503b36045820ba3a2277b6c2dc18604fd5b344ae9d9403abcbe3d55829f2a0d07cddd2503b36a4015820db0609604712bc0764c7b482c799c43bd07b127ba917e4bfa6948e11c0a4a35702a200020158205bad2dace137cf49a1d4f3f991eede29fabdac7a154c80069d9cbad6ab66b41603a4010102a100010381a300070100020004a300070100020004a200030101a4015820fa0012c40679ced195ef4240003b68b7047a978d9e7c34d8fe9bb9dbba38649d02a20001015820c7bcda64c98e778efd4ecf3e80e7cef39054a997be8db35aa2e2a91ff2d2cbc003a4010102a100010381a200030182a200010158201f735aeecea8726a3e13676d04618097f05e2b7bf0f3e428876aa2e014e0b2b5a200010158201f735aeecea8726a3e13676d04618097f05e2b7bf0f3e428876aa2e014e0b2b504a200010158201f735aeecea8726a3e13676d04618097f05e2b7bf0f3e428876aa2e014e0b2b504a2000201a4010102a1000103815820e903528dd5072ab4d40edeae0b51a27fd456c72411201ec669867b52539545b7045820ba3a2277b6c2dc18604fd5b344ae9d9403abcbe3d55829f2a0d07cddd2503b36"
    );

    assert_eq!(
        decode(&fixture.surface).validate_against(&fixture.foundation, &fixture.direct),
        Ok(fixture.surface)
    );
}

#[test]
fn callable_surface_reader_rejects_unknown_missing_and_extra_fields() {
    for bytes in [
        vec![0x81, 0xa3, 0x01, 0x40, 0x02, 0xa1, 0x00, 0x01, 0x03, 0xa0],
        vec![
            0x81, 0xa5, 0x01, 0x40, 0x02, 0xa1, 0x00, 0x01, 0x03, 0xa0, 0x04, 0xa0, 0x05, 0x00,
        ],
    ] {
        assert!(
            decode_canonical::<DecodedCoreCallableTargetSurfaceV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }

    for bytes in [
        vec![0xa1, 0x00, 0x04],
        vec![0xa1, 0x00, 0x01],
        vec![0xa3, 0x00, 0x01, 0x01, 0xa0, 0x02, 0x00],
    ] {
        assert!(
            decode_canonical::<DecodedCoreHirCallableCapabilityV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }

    for bytes in [
        vec![0xa1, 0x00, 0x03],
        vec![0xa1, 0x00, 0x01],
        vec![0xa3, 0x00, 0x01, 0x01, 0x40, 0x02, 0x00],
    ] {
        assert!(
            decode_canonical::<DecodedCoreCallableDefinitionV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }
}

#[test]
fn callable_surface_requires_canonical_total_direct_callable_coverage() {
    let fixture = fixture();
    let targets = fixture.surface.targets().to_vec();

    let missing = raw_surface(&targets[..targets.len() - 1]);
    assert_eq!(
        missing.validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreCallableTargetSurfaceValidationError::Coverage {
            expected: targets.len(),
            actual: targets.len() - 1,
        })
    );

    let mut reversed = targets.clone();
    reversed.reverse();
    assert!(matches!(
        raw_surface(&reversed).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreCallableTargetSurfaceValidationError::NonCanonicalOrder { .. })
    ));

    let mut duplicate = targets.clone();
    duplicate[1] = duplicate[0].clone();
    assert!(matches!(
        raw_surface(&duplicate).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreCallableTargetSurfaceValidationError::DuplicateBinding { .. })
    ));

    let mut unknown = targets;
    unknown[0].binding = function_binding(
        plain_function("unknown", SignatureTypeKey::Nominal(fixture.source_type)).key(),
    )
    .id();
    unknown.sort_by_key(CoreCallableTargetV1::binding);
    assert!(matches!(
        raw_surface(&unknown).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreCallableTargetSurfaceValidationError::UnknownBinding { .. })
    ));
}

#[test]
fn callable_surface_replays_definition_origin_signature_and_capability() {
    let fixture = fixture();
    let plain_index = fixture
        .surface
        .targets()
        .iter()
        .position(|target| {
            matches!(
                target.capability(),
                CoreHirCallableCapabilityV1::ParamFreeCandidate(_)
            )
        })
        .unwrap();
    let generic_index = fixture
        .surface
        .targets()
        .iter()
        .position(|target| {
            matches!(
                target.capability(),
                CoreHirCallableCapabilityV1::GenericUnavailable { .. }
            )
        })
        .unwrap();

    let mut wrong_definition = fixture.surface.targets().to_vec();
    wrong_definition[plain_index].definition = wrong_definition[generic_index].definition;
    assert!(matches!(
        raw_surface(&wrong_definition).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreCallableTargetSurfaceValidationError::DefinitionMismatch(_))
    ));

    let mut without_origin = fixture.foundation.clone();
    without_origin.set_definition_origins(Vec::new()).unwrap();
    assert!(matches!(
        decode(&fixture.surface).validate_against(&without_origin, &fixture.direct),
        Err(CoreCallableTargetSurfaceValidationError::MissingDefinitionOrigin(_))
    ));

    let mut wrong_signature = fixture.surface.targets().to_vec();
    wrong_signature[plain_index].signature = SignatureCallableShape::new(
        Effect::Ordinary,
        None,
        Vec::new(),
        SignatureTypeKey::Nominal(fixture.source_type),
    );
    assert!(matches!(
        raw_surface(&wrong_signature).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreCallableTargetSurfaceValidationError::SignatureMismatch(
            _
        ))
    ));

    let mut wrong_capability = fixture.surface.targets().to_vec();
    let CoreHirCallableCapabilityV1::ParamFreeCandidate(exact) =
        wrong_capability[plain_index].capability.clone()
    else {
        panic!("fixture must contain a param-free Strong callable")
    };
    wrong_capability[plain_index].capability =
        CoreHirCallableCapabilityV1::StructuralUnavailable(exact);
    assert!(matches!(
        raw_surface(&wrong_capability).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreCallableTargetSurfaceValidationError::CapabilityMismatch(_))
    ));

    let mut wrong_generic_count = fixture.surface.targets().to_vec();
    wrong_generic_count[generic_index].capability =
        CoreHirCallableCapabilityV1::GenericUnavailable {
            type_parameter_count: 2,
        };
    assert!(matches!(
        raw_surface(&wrong_generic_count).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreCallableTargetSurfaceValidationError::CapabilityMismatch(_))
    ));

    let unknown_type =
        CborIdentityRecord::<PersistentTypeId, _>::from_key(SourceDeclarationKey::nominal(
            site(),
            CanonicalIdentifier::new("Unknown").unwrap(),
            SourceNominalKind::Class,
            0,
        ))
        .unwrap();
    let unknown_exact = CborIdentityRecord::<PersistentExactTypeId, _>::from_key(
        ExactTypeKey::Nominal(unknown_type.id()),
    )
    .unwrap();
    let mut unknown_exact_signature = fixture.surface.targets().to_vec();
    unknown_exact_signature[plain_index].capability =
        CoreHirCallableCapabilityV1::ParamFreeCandidate(ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            vec![unknown_exact.id()],
            unknown_exact.id(),
        ));
    assert!(matches!(
        raw_surface(&unknown_exact_signature)
            .validate_against(&fixture.foundation, &fixture.direct),
        Err(
            CoreCallableTargetSurfaceValidationError::SignatureReference(
                CoreCallableSignatureReferenceError::UnknownExactType(_)
            )
        )
    ));
}

#[test]
fn callable_signature_binders_are_local_and_within_declared_arity() {
    let shape = |binder: SignatureTypeKey| {
        SignatureCallableShape::new(Effect::Ordinary, None, vec![binder.clone()], binder)
    };
    assert_eq!(
        validate_binders(&shape(SignatureTypeKey::Binder { depth: 1, index: 0 }), 1),
        Err(CoreCallableBinderError::NonLocalDepth(1))
    );
    assert_eq!(
        validate_binders(&shape(SignatureTypeKey::Binder { depth: 0, index: 1 }), 1),
        Err(CoreCallableBinderError::IndexOutOfRange {
            index: 1,
            type_parameter_count: 1,
        })
    );
}

fn decode(surface: &CoreCallableTargetSurfaceV1) -> DecodedCoreCallableTargetSurfaceV1 {
    decode_canonical(&encode(surface).unwrap(), DecodeLimits::default()).unwrap()
}

fn raw_surface(targets: &[CoreCallableTargetV1]) -> DecodedCoreCallableTargetSurfaceV1 {
    struct RawSurface<'a>(&'a [CoreCallableTargetV1]);

    impl WireEncode for RawSurface<'_> {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.array(self.0.len() as u64)?;
            for target in self.0 {
                target.encode(encoder)?;
            }
            Ok(())
        }
    }

    decode_canonical(
        &encode(&RawSurface(targets)).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
}

struct Fixture {
    foundation: CanonicalHirFoundation,
    direct: CanonicalDirectPublicSurfaceV1,
    surface: CoreCallableTargetSurfaceV1,
    source_type: PersistentTypeId,
}

fn fixture() -> Fixture {
    let source_type_key = SourceDeclarationKey::nominal(
        site(),
        CanonicalIdentifier::new("Text").unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let source_type_record =
        CborIdentityRecord::<PersistentTypeId, _>::from_key(source_type_key).unwrap();
    let source_type = source_type_record.id();
    let nominal_exact = CborIdentityRecord::<PersistentExactTypeId, _>::from_key(
        ExactTypeKey::Nominal(source_type),
    )
    .unwrap();
    let tuple_exact =
        CborIdentityRecord::<PersistentExactTypeId, _>::from_key(ExactTypeKey::Tuple(
            NonEmptyVec::new(vec![nominal_exact.id(), nominal_exact.id()]).unwrap(),
        ))
        .unwrap();

    let plain = plain_function("plain", SignatureTypeKey::Nominal(source_type));
    let structural = plain_function(
        "structural",
        SignatureTypeKey::Tuple(
            NonEmptyVec::new(vec![
                SignatureTypeKey::Nominal(source_type),
                SignatureTypeKey::Nominal(source_type),
            ])
            .unwrap(),
        ),
    );
    let generic = generic_function("generic");

    let plain_binding = function_binding(plain.key());
    let structural_binding = function_binding(structural.key());
    let generic_binding = function_binding(generic.key());

    let plain_target = CoreCallableTargetV1 {
        binding: plain_binding.id(),
        definition: CoreCallableDefinitionV1::Function(plain.id()),
        signature: SignatureCallableShape::new(
            Effect::Ordinary,
            None,
            vec![SignatureTypeKey::Nominal(source_type)],
            SignatureTypeKey::Nominal(source_type),
        ),
        capability: CoreHirCallableCapabilityV1::ParamFreeCandidate(ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            vec![nominal_exact.id()],
            nominal_exact.id(),
        )),
    };
    let structural_target = CoreCallableTargetV1 {
        binding: structural_binding.id(),
        definition: CoreCallableDefinitionV1::Function(structural.id()),
        signature: SignatureCallableShape::new(
            Effect::Ordinary,
            None,
            vec![SignatureTypeKey::Tuple(
                NonEmptyVec::new(vec![
                    SignatureTypeKey::Nominal(source_type),
                    SignatureTypeKey::Nominal(source_type),
                ])
                .unwrap(),
            )],
            SignatureTypeKey::Nominal(source_type),
        ),
        capability: CoreHirCallableCapabilityV1::StructuralUnavailable(
            ExactCallableSignature::new(
                Effect::Ordinary,
                None,
                vec![tuple_exact.id()],
                nominal_exact.id(),
            ),
        ),
    };
    let binder = SignatureTypeKey::Binder { depth: 0, index: 0 };
    let generic_target = CoreCallableTargetV1 {
        binding: generic_binding.id(),
        definition: CoreCallableDefinitionV1::GenericFunction(generic.id()),
        signature: SignatureCallableShape::new(
            Effect::Ordinary,
            None,
            vec![binder.clone()],
            binder,
        ),
        capability: CoreHirCallableCapabilityV1::GenericUnavailable {
            type_parameter_count: 1,
        },
    };
    let surface =
        CoreCallableTargetSurfaceV1::try_new(vec![generic_target, structural_target, plain_target])
            .unwrap();

    let mut foundation = CanonicalHirFoundation::empty();
    foundation.set_types(vec![source_type_record]).unwrap();
    foundation
        .set_functions(vec![plain.clone(), structural.clone()])
        .unwrap();
    foundation
        .set_generic_functions(vec![generic.clone()])
        .unwrap();
    foundation
        .set_exact_types(vec![tuple_exact, nominal_exact])
        .unwrap();
    foundation
        .set_export_bindings(vec![generic_binding, structural_binding, plain_binding])
        .unwrap();
    foundation
        .set_definition_origins(vec![
            origin_record(DefinitionOriginSubject::Function(plain.id())),
            origin_record(DefinitionOriginSubject::Function(structural.id())),
            origin_record(DefinitionOriginSubject::GenericFunction(generic.id())),
        ])
        .unwrap();
    let direct = CanonicalDirectPublicSurfaceV1::try_new(
        surface
            .targets()
            .iter()
            .map(CoreCallableTargetV1::binding)
            .collect(),
    )
    .unwrap();
    Fixture {
        foundation,
        direct,
        surface,
        source_type,
    }
}

fn plain_function(
    name: &str,
    parameter: SignatureTypeKey,
) -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        site(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        vec![parameter],
    ))
    .unwrap()
}

fn generic_function(
    name: &str,
) -> CborIdentityRecord<PersistentGenericFunctionId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        site(),
        CanonicalIdentifier::new(name).unwrap(),
        1,
        None,
        vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
    ))
    .unwrap()
}

fn function_binding(
    declaration: &SourceDeclarationKey,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    let target = BindingTarget::function(declaration).unwrap();
    CborIdentityRecord::from_key(ExportBindingKey::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        match declaration.name() {
            scoop_identity::DeclarationName::Named(name) => name.clone(),
            scoop_identity::DeclarationName::Constructor => {
                panic!("fixture function must be named")
            }
        },
        target,
    ))
    .unwrap()
}

fn origin_record(subject: DefinitionOriginSubject) -> DefinitionOriginRecord {
    let source = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/core.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    let origin = DefinitionOrigin::new(source, SourceSpan::new(0, 0).unwrap(), &context).unwrap();
    DefinitionOriginRecord::new(subject, origin)
}

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
