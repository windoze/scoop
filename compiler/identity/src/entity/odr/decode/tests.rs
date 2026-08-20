use scoop_wire::{Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedOdrMemberDiscriminator, DecodedOdrMemberKey, DecodedSpecializationKey,
    OdrIdentityResolutionError,
};
use crate::{
    CallableApplicationKey, CallableInstantiationOwner, CborIdentityRecord,
    DecodedCborIdentityRecord, NonEmptyVec, OdrGroupId, OdrMemberDiscriminator, OdrMemberId,
    OdrMemberIdentityError, OdrMemberKey, OdrMemberRole, PersistentCallableApplicationId,
    PersistentCallableBodyId, PersistentConstructorId, PersistentDispatchSlotId,
    PersistentDispatchTableId, PersistentEnumVariantId, PersistentExactTypeId,
    PersistentExtensionPropertyId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentIdMismatch,
    PersistentIdResolver, PersistentImmortalObjectId, PersistentInitializationUnitId,
    PersistentLayoutId, PersistentPropertyAccessorId, PersistentSafepointSiteId, PersistentScanId,
    PersistentStaticStorageId, PersistentTypeId, SpecializationKey, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent from the test graph")
    }
}

impl std::error::Error for ResolutionError {}

struct Resolver {
    group: OdrGroupId,
}

struct RawOdrMemberKey {
    group: OdrGroupId,
    role: OdrMemberRole,
    discriminator: OdrMemberDiscriminator,
}

impl WireEncode for RawOdrMemberKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.group.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        self.discriminator.encode(encoder)
    }
}

macro_rules! id_resolver {
    ($id:ty, $expected:ident) => {
        impl PersistentIdResolver<$id> for Resolver {
            type Error = ResolutionError;

            fn resolve(&mut self, id: crate::DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                id.verify($expected())
                    .map_err(|_: PersistentIdMismatch<$id>| ResolutionError)
            }
        }
    };
}

id_resolver!(PersistentGenericTypeId, generic_type);
id_resolver!(PersistentExactTypeId, exact_type);
id_resolver!(PersistentFunctionId, function);
id_resolver!(PersistentGenericFunctionId, generic_function);
id_resolver!(PersistentConstructorId, constructor);
id_resolver!(PersistentEnumVariantId, enum_variant);
id_resolver!(PersistentPropertyAccessorId, accessor);
id_resolver!(PersistentCallableApplicationId, callable_application);
id_resolver!(PersistentInitializationUnitId, initialization_unit);
id_resolver!(PersistentExtensionPropertyId, extension_property);
id_resolver!(PersistentGeneratedCallableId, generated_callable);
id_resolver!(PersistentTypeId, nominal_type);
id_resolver!(PersistentLayoutId, layout);
id_resolver!(PersistentScanId, scan);
id_resolver!(PersistentDispatchTableId, dispatch_table);
id_resolver!(PersistentDispatchSlotId, dispatch_slot);
id_resolver!(PersistentStaticStorageId, static_storage);
id_resolver!(PersistentImmortalObjectId, immortal_object);
id_resolver!(PersistentCallableBodyId, callable_body);
id_resolver!(PersistentSafepointSiteId, safepoint_site);

impl PersistentIdResolver<OdrGroupId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<OdrGroupId>,
    ) -> Result<OdrGroupId, Self::Error> {
        id.verify(self.group)
            .map_err(|_: PersistentIdMismatch<OdrGroupId>| ResolutionError)
    }
}

#[test]
fn every_specialization_shape_round_trips_and_validates_its_record() {
    for key in specialization_keys() {
        let record = CborIdentityRecord::<OdrGroupId, _>::from_key(key.clone()).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<OdrGroupId, DecodedSpecializationKey>,
        >(&encode(&record).unwrap())
        .unwrap();
        let resolved = decoded
            .resolve(|key| key.resolve(&mut resolver(record.id())))
            .unwrap();

        assert_eq!(resolved, record);
    }
}

#[test]
fn every_member_role_and_discriminator_round_trips_and_validates_its_record() {
    let group = group();
    for (role, discriminator) in member_cases() {
        let key = OdrMemberKey::new(group, role, discriminator).unwrap();
        let record = CborIdentityRecord::<OdrMemberId, _>::from_key(key).unwrap();
        let decoded =
            decode_canonical::<DecodedCborIdentityRecord<OdrMemberId, DecodedOdrMemberKey>>(
                &encode(&record).unwrap(),
            )
            .unwrap();
        let resolved = decoded
            .resolve(|key| key.resolve(&mut resolver(group)))
            .unwrap();

        assert_eq!(resolved, record);
    }
}

#[test]
fn member_resolution_rechecks_the_role_discriminator_matrix() {
    let raw = RawOdrMemberKey {
        group: group(),
        role: OdrMemberRole::TypeDescriptor,
        discriminator: OdrMemberDiscriminator::Layout(layout()),
    };
    let decoded = decode_canonical::<DecodedOdrMemberKey>(&encode(&raw).unwrap()).unwrap();

    assert_eq!(
        decoded.resolve(&mut resolver(group())),
        Err(OdrIdentityResolutionError::Member(
            OdrMemberIdentityError::RoleDiscriminatorMismatch
        ))
    );
}

#[test]
fn specialization_resolution_rejects_a_reference_from_another_identity_graph() {
    let key = SpecializationKey::Nominal {
        origin: PersistentGenericTypeId([99; 32]),
        arguments: NonEmptyVec::from_first(exact_type(), []),
    };
    let decoded = decode_canonical::<DecodedSpecializationKey>(&encode(&key).unwrap()).unwrap();

    assert_eq!(
        decoded.resolve(&mut resolver(group())),
        Err(OdrIdentityResolutionError::Reference(ResolutionError))
    );
}

#[test]
fn decoders_reject_empty_specialization_arguments_and_unknown_tags() {
    let mut empty_arguments = vec![0xa3, 0x00, 0x01, 0x01, 0x58, 0x20];
    empty_arguments.extend_from_slice(generic_type().as_array());
    empty_arguments.extend_from_slice(&[0x02, 0x80]);
    let error = decode_canonical::<DecodedSpecializationKey>(&empty_arguments).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 0,
        }
    );

    let error = decode_canonical::<DecodedSpecializationKey>(b"\xa1\x00\x05").unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 5 });

    let error = decode_canonical::<OdrMemberRole>(b"\x0b").unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 11 });

    let error = decode_canonical::<OdrMemberRole>(b"\x11").unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 17 });

    let error = decode_canonical::<DecodedOdrMemberDiscriminator>(b"\xa1\x00\x10").unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 16 });
}

fn resolver(group: OdrGroupId) -> Resolver {
    Resolver { group }
}

fn specialization_keys() -> [SpecializationKey; 4] {
    [
        SpecializationKey::Nominal {
            origin: generic_type(),
            arguments: NonEmptyVec::from_first(exact_type(), []),
        },
        SpecializationKey::Callable {
            application: CallableApplicationKey::for_function(
                function(),
                CallableInstantiationOwner::NoOwner,
            ),
        },
        SpecializationKey::DelegatedProperty {
            origin: extension_property(),
            receiver_arguments: NonEmptyVec::from_first(exact_type(), []),
        },
        SpecializationKey::StructuralType {
            exact_type: exact_type(),
        },
    ]
}

fn member_cases() -> Vec<(OdrMemberRole, OdrMemberDiscriminator)> {
    use OdrMemberDiscriminator as D;
    use OdrMemberRole as R;

    vec![
        (
            R::CallableBody,
            D::CallableApplication(callable_application()),
        ),
        (R::CallableBody, D::GeneratedCallable(generated_callable())),
        (
            R::CallableBody,
            D::InitializationUnit(initialization_unit()),
        ),
        (R::GeneratedNominal, D::GeneratedNominal(nominal_type())),
        (R::Layout, D::Layout(layout())),
        (R::ScanProgram, D::Scan(scan())),
        (R::TypeDescriptor, D::ExactType(exact_type())),
        (R::DispatchTable, D::DispatchTable(dispatch_table())),
        (R::DispatchAdapter, D::DispatchSlot(dispatch_slot())),
        (R::StaticStorage, D::StaticStorage(static_storage())),
        (R::ImmortalObject, D::ImmortalObject(immortal_object())),
        (
            R::InitializationCell,
            D::InitializationUnit(initialization_unit()),
        ),
        (R::RegistrationRecord, D::CallableBody(callable_body())),
        (R::RegistrationRecord, D::SafepointSite(safepoint_site())),
        (R::DiagnosticBytes, D::StructuralPath(structural_path())),
        (
            R::AddressTakenConstant,
            D::ImmortalObject(immortal_object()),
        ),
        (R::ObjectSupport, D::Singleton),
        (R::ReleaseHook, D::ExactType(exact_type())),
    ]
}

fn structural_path() -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::SyntheticValue, 3),
        [],
    )
}

const fn generic_type() -> PersistentGenericTypeId {
    PersistentGenericTypeId([1; 32])
}

const fn exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId([2; 32])
}

const fn function() -> PersistentFunctionId {
    PersistentFunctionId([3; 32])
}

const fn generic_function() -> PersistentGenericFunctionId {
    PersistentGenericFunctionId([4; 32])
}

const fn constructor() -> PersistentConstructorId {
    PersistentConstructorId([5; 32])
}

const fn enum_variant() -> PersistentEnumVariantId {
    PersistentEnumVariantId([23; 32])
}

const fn accessor() -> PersistentPropertyAccessorId {
    PersistentPropertyAccessorId([6; 32])
}

const fn callable_application() -> PersistentCallableApplicationId {
    PersistentCallableApplicationId([7; 32])
}

const fn initialization_unit() -> PersistentInitializationUnitId {
    PersistentInitializationUnitId([8; 32])
}

const fn extension_property() -> PersistentExtensionPropertyId {
    PersistentExtensionPropertyId([9; 32])
}

const fn generated_callable() -> PersistentGeneratedCallableId {
    PersistentGeneratedCallableId([10; 32])
}

const fn nominal_type() -> PersistentTypeId {
    PersistentTypeId([11; 32])
}

const fn layout() -> PersistentLayoutId {
    PersistentLayoutId([12; 32])
}

const fn scan() -> PersistentScanId {
    PersistentScanId([13; 32])
}

const fn dispatch_table() -> PersistentDispatchTableId {
    PersistentDispatchTableId([14; 32])
}

const fn dispatch_slot() -> PersistentDispatchSlotId {
    PersistentDispatchSlotId([15; 32])
}

const fn static_storage() -> PersistentStaticStorageId {
    PersistentStaticStorageId([16; 32])
}

const fn immortal_object() -> PersistentImmortalObjectId {
    PersistentImmortalObjectId([17; 32])
}

const fn callable_body() -> PersistentCallableBodyId {
    PersistentCallableBodyId([18; 32])
}

const fn safepoint_site() -> PersistentSafepointSiteId {
    PersistentSafepointSiteId([19; 32])
}

const fn group() -> OdrGroupId {
    OdrGroupId([20; 32])
}
