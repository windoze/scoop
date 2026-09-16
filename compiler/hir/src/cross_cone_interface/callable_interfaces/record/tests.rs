use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    DeclarationScope, DefinitionOwnerAtom, DefinitionOwnerChain, Effect, GcEffect,
    IdentityReferenceError, NominalDeclarationOwner, PackagePath, PersistentConstructorId,
    PersistentFunctionId, PersistentGenericFunctionId, PersistentTypeId, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::*;
use crate::{
    CallableInfixV1, CallableOperatorRoleV1, CallableSafetyV1, CallableSourceEffectsBuildError,
    CanonicalBinderListV1, SourceParameterShapeV1, TypeParameterBinderV1, TypeParameterBoundsV1,
};

#[test]
fn callable_record_has_fixed_field_wire_and_accessors() {
    let fixture = Fixture::new();
    let record = fixture.record();
    let expected = [
        b"\xa9\x01".as_slice(),
        encode(&record.declaration()).unwrap().as_slice(),
        b"\x02".as_slice(),
        encode(&record.owner()).unwrap().as_slice(),
        b"\x03".as_slice(),
        encode(record.type_parameters()).unwrap().as_slice(),
        b"\x04".as_slice(),
        encode(&scoop_identity::OptionalSignatureType::from_option(
            record.receiver().cloned(),
        ))
        .unwrap()
        .as_slice(),
        b"\x05".as_slice(),
        encode(record.parameters()).unwrap().as_slice(),
        b"\x06".as_slice(),
        encode(record.result()).unwrap().as_slice(),
        b"\x07".as_slice(),
        encode(&record.effects()).unwrap().as_slice(),
        b"\x08".as_slice(),
        encode(&record.modality()).unwrap().as_slice(),
        b"\x09".as_slice(),
        encode(&record.access()).unwrap().as_slice(),
    ]
    .concat();

    assert_eq!(encode(&record).unwrap(), expected);
    assert_eq!(
        record.declaration(),
        CallableTemplateOrigin::GenericFunction(fixture.generic_function.id())
    );
    assert_eq!(record.owner(), PublicDeclarationOwnerV1::Extension);
    assert_eq!(record.type_parameters().len_u32(), 1);
    assert_eq!(record.receiver(), Some(&binder(0)));
    assert_eq!(record.parameters().len_u32(), 1);
    assert_eq!(record.result(), &binder(0));
    assert_eq!(record.effects(), scoop_effects());
    assert_eq!(record.modality(), CallableModalityV1::Final);
    assert_eq!(record.access(), PublicLookupAccessV1::DirectOnly);
}

#[test]
fn decoded_record_resolves_every_typed_constituent() {
    let fixture = Fixture::new();
    let expected = fixture.record();
    let decoded = decode_record(&expected);
    let mut authority = fixture.authority();

    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
}

#[test]
fn producer_rejects_declaration_binder_shape_mismatches() {
    let fixture = Fixture::new();
    assert_eq!(
        build_record(
            CallableTemplateOrigin::GenericFunction(fixture.generic_function.id()),
            PublicDeclarationOwnerV1::Extension,
            empty_binders(),
            Some(binder(0)),
            scoop_effects(),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
        ),
        Err(CallableInterfaceRecordBuildError::MissingTypeParameters(
            CallableTemplateOrigin::GenericFunction(fixture.generic_function.id())
        ))
    );
    assert_eq!(
        build_record(
            CallableTemplateOrigin::Function(fixture.function.id()),
            PublicDeclarationOwnerV1::TopLevel,
            one_binder(),
            None,
            scoop_effects(),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
        ),
        Err(CallableInterfaceRecordBuildError::UnexpectedTypeParameters(
            CallableTemplateOrigin::Function(fixture.function.id())
        ))
    );
}

#[test]
fn producer_rejects_owner_and_receiver_shape_mismatches() {
    let fixture = Fixture::new();
    assert_eq!(
        build_record(
            CallableTemplateOrigin::GenericFunction(fixture.generic_function.id()),
            PublicDeclarationOwnerV1::Extension,
            one_binder(),
            None,
            scoop_effects(),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
        ),
        Err(CallableInterfaceRecordBuildError::MissingExtensionReceiver)
    );
    assert_eq!(
        build_record(
            CallableTemplateOrigin::Function(fixture.function.id()),
            PublicDeclarationOwnerV1::TopLevel,
            empty_binders(),
            Some(binder(0)),
            scoop_effects(),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
        ),
        Err(CallableInterfaceRecordBuildError::UnexpectedReceiver(
            PublicDeclarationOwnerV1::TopLevel
        ))
    );
    assert_eq!(
        build_record(
            CallableTemplateOrigin::Constructor(fixture.constructor.id()),
            PublicDeclarationOwnerV1::TopLevel,
            empty_binders(),
            None,
            scoop_effects(),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
        ),
        Err(CallableInterfaceRecordBuildError::NominalOwnerRequired {
            declaration: CallableTemplateOrigin::Constructor(fixture.constructor.id()),
            actual: PublicDeclarationOwnerV1::TopLevel,
        })
    );
}

#[test]
fn producer_rejects_invalid_dispatch_and_extern_contracts() {
    let fixture = Fixture::new();
    let nominal_owner =
        PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::Concrete(fixture.nominal.id()));
    assert_eq!(
        build_record(
            CallableTemplateOrigin::Function(fixture.function.id()),
            nominal_owner,
            empty_binders(),
            None,
            scoop_effects(),
            CallableModalityV1::Open,
            PublicLookupAccessV1::DirectOnly,
        ),
        Err(CallableInterfaceRecordBuildError::SlotAccessRequired(
            CallableModalityV1::Open
        ))
    );
    assert!(matches!(
        build_record(
            CallableTemplateOrigin::Function(fixture.function.id()),
            PublicDeclarationOwnerV1::TopLevel,
            empty_binders(),
            None,
            scoop_effects(),
            CallableModalityV1::Final,
            PublicLookupAccessV1::PublicSlot,
        ),
        Err(CallableInterfaceRecordBuildError::DirectCallableContract { .. })
    ));
    assert_eq!(
        build_record(
            CallableTemplateOrigin::Function(fixture.function.id()),
            PublicDeclarationOwnerV1::TopLevel,
            empty_binders(),
            None,
            scoop_effects(),
            CallableModalityV1::InterfaceDefault,
            PublicLookupAccessV1::PublicSlot,
        ),
        Err(CallableInterfaceRecordBuildError::NominalOwnerRequired {
            declaration: CallableTemplateOrigin::Function(fixture.function.id()),
            actual: PublicDeclarationOwnerV1::TopLevel,
        })
    );
    assert_eq!(
        build_record(
            CallableTemplateOrigin::GenericFunction(fixture.generic_function.id()),
            PublicDeclarationOwnerV1::Extension,
            one_binder(),
            Some(binder(0)),
            extern_effects(),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
        ),
        Err(CallableInterfaceRecordBuildError::InvalidExternTarget {
            declaration: CallableTemplateOrigin::GenericFunction(fixture.generic_function.id()),
            owner: PublicDeclarationOwnerV1::Extension,
        })
    );
}

#[test]
fn reader_replays_effect_and_record_invariants() {
    let fixture = Fixture::new();
    let record = fixture.record();
    let decoded: DecodedCallableInterfaceRecordV1 = decode_canonical(
        &encode(&InvalidEffectsRecord(&record)).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let mut authority = fixture.authority();
    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(CallableInterfaceRecordResolutionError::Effects(
            CallableSourceEffectsBuildError::NoGcSuspend
        ))
    ));

    let mut decoded = decode_record(&record);
    decoded.access = PublicLookupAccessV1::PublicSlot;
    let mut authority = fixture.authority();
    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(CallableInterfaceRecordResolutionError::Record(
            CallableInterfaceRecordBuildError::DirectCallableContract { .. }
        ))
    ));
}

#[test]
fn reader_reports_missing_typed_declaration_authority() {
    let fixture = Fixture::new();
    let mut empty = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();

    assert!(matches!(
        decode_record(&fixture.record()).resolve(&mut empty),
        Err(CallableInterfaceRecordResolutionError::Declaration(
            IdentityReferenceError::Missing { .. }
        ))
    ));
}

#[test]
fn reader_requires_the_exact_record_map_shape() {
    let error =
        decode_canonical::<DecodedCallableInterfaceRecordV1>(&[0xa0], DecodeLimits::default())
            .unwrap_err();

    assert!(matches!(
        error.kind(),
        WireErrorKind::InvalidLength {
            expected: 9,
            actual: 0,
        }
    ));
}

struct Fixture {
    generic_function: CborIdentityRecord<PersistentGenericFunctionId, SourceDeclarationKey>,
    function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    nominal: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    constructor: CborIdentityRecord<PersistentConstructorId, SourceDeclarationKey>,
}

impl Fixture {
    fn new() -> Self {
        let generic_function = CborIdentityRecord::from_key(SourceDeclarationKey::function(
            top_level_site(),
            identifier("transform"),
            1,
            Some(binder(0)),
            vec![binder(0)],
        ))
        .unwrap();
        let function = CborIdentityRecord::from_key(SourceDeclarationKey::function(
            top_level_site(),
            identifier("inspect"),
            0,
            None,
            vec![binder(0)],
        ))
        .unwrap();
        let nominal_key = SourceDeclarationKey::nominal(
            top_level_site(),
            identifier("Container"),
            SourceNominalKind::Class,
            0,
        );
        let nominal = CborIdentityRecord::from_key(nominal_key).unwrap();
        let constructor = CborIdentityRecord::from_key(SourceDeclarationKey::constructor(
            owned_site(DefinitionOwnerAtom::Type(nominal.id())),
            Vec::new(),
        ))
        .unwrap();
        Self {
            generic_function,
            function,
            nominal,
            constructor,
        }
    }

    fn record(&self) -> CallableInterfaceRecordV1 {
        build_record(
            CallableTemplateOrigin::GenericFunction(self.generic_function.id()),
            PublicDeclarationOwnerV1::Extension,
            one_binder(),
            Some(binder(0)),
            scoop_effects(),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
        )
        .unwrap()
    }

    fn authority(&self) -> scoop_identity::ValidatedIdentityGraph {
        let mut pending = scoop_identity::PendingIdentityValidation::new();
        pending
            .register_external_canonical_authority(self.generic_function.clone())
            .unwrap();
        pending.finish().unwrap()
    }
}

#[allow(clippy::too_many_arguments)]
fn build_record(
    declaration: CallableTemplateOrigin,
    owner: PublicDeclarationOwnerV1,
    type_parameters: CanonicalBinderListV1,
    receiver: Option<SignatureTypeKey>,
    effects: CallableSourceEffectsV1,
    modality: CallableModalityV1,
    access: PublicLookupAccessV1,
) -> Result<CallableInterfaceRecordV1, CallableInterfaceRecordBuildError> {
    CallableInterfaceRecordV1::try_new(
        declaration,
        owner,
        type_parameters,
        receiver,
        CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
            identifier("value"),
            binder(0),
        )])
        .unwrap(),
        binder(0),
        effects,
        modality,
        access,
    )
}

fn decode_record(value: &CallableInterfaceRecordV1) -> DecodedCallableInterfaceRecordV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

fn one_binder() -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
        identifier("T"),
        TypeParameterBoundsV1::Unconstrained,
    )])
    .unwrap()
}

fn empty_binders() -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(Vec::new()).unwrap()
}

fn scoop_effects() -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        CallableSafetyV1::Safe,
        GcEffect::Managed,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}

fn extern_effects() -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        CallableSafetyV1::Safe,
        GcEffect::Managed,
        CallableImplementationV1::SourceExternScoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}

fn top_level_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn owned_site(owner: DefinitionOwnerAtom) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(vec![owner]),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

const fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

struct InvalidEffectsRecord<'record>(&'record CallableInterfaceRecordV1);

impl WireEncode for InvalidEffectsRecord<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let record = self.0;
        encoder.map(9)?;
        encoder.field(1)?;
        record.declaration.encode(encoder)?;
        encoder.field(2)?;
        record.owner.encode(encoder)?;
        encoder.field(3)?;
        record.type_parameters.encode(encoder)?;
        encoder.field(4)?;
        scoop_identity::OptionalSignatureType::from_option(record.receiver.clone())
            .encode(encoder)?;
        encoder.field(5)?;
        record.parameters.encode(encoder)?;
        encoder.field(6)?;
        record.result.encode(encoder)?;
        encoder.field(7)?;
        encode_invalid_no_gc_suspend_effects(encoder)?;
        encoder.field(8)?;
        record.modality.encode(encoder)?;
        encoder.field(9)?;
        record.access.encode(encoder)
    }
}

fn encode_invalid_no_gc_suspend_effects(
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(6)?;
    encoder.field(1)?;
    Effect::Suspend.encode(encoder)?;
    encoder.field(2)?;
    CallableSafetyV1::Safe.encode(encoder)?;
    encoder.field(3)?;
    GcEffect::NoGc.encode(encoder)?;
    encoder.field(4)?;
    CallableImplementationV1::Scoop.encode(encoder)?;
    encoder.field(5)?;
    CallableOperatorRoleV1::None.encode(encoder)?;
    encoder.field(6)?;
    CallableInfixV1::Ordinary.encode(encoder)
}
