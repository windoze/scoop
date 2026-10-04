use scoop_wire::{Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedDefinitionAtomSubkey, DecodedObjectDefinitionAtomKey, DecodedObjectDefinitionPlanKey,
    DecodedObjectDefinitionPlanOwner, DecodedStrongDefinitionEntity,
    ObjectDefinitionResolutionError,
};
use crate::{
    CanonicalCAbiLayoutFingerprint, CborIdentityRecord, ConeIdentity, ConeImageSupportRole,
    DecodedCborIdentityRecord, DefinitionAtomRole, DefinitionAtomSubkey, GeneratedBridgeAtomId,
    GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey, GeneratedBridgeUnitId,
    ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionIdentityError,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, ObjectDefinitionPlanOwner,
    ObjectDefinitionPlanRole, OdrMemberId, PersistentCallableBodyId, PersistentDispatchSlotId,
    PersistentDispatchTableId, PersistentExactTypeId, PersistentIdMismatch, PersistentIdResolver,
    PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentKeyResolver,
    PersistentLayoutId, PersistentSafepointSiteId, PersistentScanId, PersistentStaticStorageId,
    StrongDefinitionEntity, StrongDefinitionRole, StructuralDefinitionPath,
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
    bridge: GeneratedBridgeAtomKey,
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

id_resolver!(ConeIdentity, producer);
id_resolver!(PersistentCallableBodyId, callable_body);
id_resolver!(PersistentStaticStorageId, static_storage);
id_resolver!(PersistentImmortalObjectId, immortal_object);
id_resolver!(PersistentExactTypeId, exact_type);
id_resolver!(PersistentLayoutId, layout);
id_resolver!(PersistentScanId, scan);
id_resolver!(PersistentDispatchTableId, dispatch_table);
id_resolver!(PersistentDispatchSlotId, dispatch_slot);
id_resolver!(PersistentInitializationUnitId, initialization_unit);
id_resolver!(PersistentSafepointSiteId, safepoint_site);
id_resolver!(OdrMemberId, odr_member);
id_resolver!(ObjectDefinitionPlanId, definition_plan);

impl PersistentKeyResolver<GeneratedBridgeAtomId, GeneratedBridgeAtomKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: crate::DecodedPersistentId<GeneratedBridgeAtomId>,
    ) -> Result<std::sync::Arc<GeneratedBridgeAtomKey>, Self::Error> {
        let expected =
            GeneratedBridgeAtomId::from_key(&self.bridge).map_err(|_| ResolutionError)?;
        id.verify(expected)
            .map_err(|_: PersistentIdMismatch<GeneratedBridgeAtomId>| ResolutionError)?;
        Ok(std::sync::Arc::new(self.bridge))
    }
}

#[test]
fn every_strong_plan_shape_round_trips_and_validates_its_record() {
    let bridge = materialized_bridge();
    let bridge_entity = StrongDefinitionEntity::generated_bridge_atom(&bridge).unwrap();

    for (entity, role) in strong_plan_cases(bridge_entity) {
        let key = ObjectDefinitionPlanKey::strong(producer(), entity, role).unwrap();
        let record = CborIdentityRecord::<ObjectDefinitionPlanId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<ObjectDefinitionPlanId, DecodedObjectDefinitionPlanKey>,
        >(&encode(&record).unwrap())
        .unwrap();
        let resolved = decoded
            .resolve(|key| key.resolve(&mut resolver(bridge)))
            .unwrap();

        assert_eq!(resolved, record);
    }
}

#[test]
fn odr_plan_round_trips_without_becoming_a_strong_plan() {
    let key = ObjectDefinitionPlanKey::odr(odr_member());
    let record = CborIdentityRecord::<ObjectDefinitionPlanId, _>::from_key(key).unwrap();
    let decoded = decode_canonical::<
        DecodedCborIdentityRecord<ObjectDefinitionPlanId, DecodedObjectDefinitionPlanKey>,
    >(&encode(&record).unwrap())
    .unwrap();

    assert_eq!(
        decoded
            .resolve(|key| key.resolve(&mut resolver(materialized_bridge())))
            .unwrap(),
        record
    );
}

#[test]
fn every_atom_role_and_subkey_round_trips_and_validates_its_record() {
    for (role, subkey) in atom_cases() {
        let key = ObjectDefinitionAtomKey::new(definition_plan(), role, subkey);
        let record = CborIdentityRecord::<ObjectDefinitionAtomId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<ObjectDefinitionAtomId, DecodedObjectDefinitionAtomKey>,
        >(&encode(&record).unwrap())
        .unwrap();
        let resolved = decoded
            .resolve(|key| key.resolve(&mut resolver(materialized_bridge())))
            .unwrap();

        assert_eq!(resolved, record);
    }
}

#[test]
fn plan_resolution_rechecks_owner_role_and_entity_role_matrices() {
    let strong_owner = ObjectDefinitionPlanOwner::Strong {
        producer: producer(),
        entity: StrongDefinitionEntity::layout(layout()),
    };
    let wrong_entity_role = RawPlanKey {
        owner: RawPlanOwner::Trusted(strong_owner),
        role: ObjectDefinitionPlanRole::Strong(StrongDefinitionRole::ScanProgram),
    };
    let decoded =
        decode_canonical::<DecodedObjectDefinitionPlanKey>(&encode(&wrong_entity_role).unwrap())
            .unwrap();
    assert_eq!(
        decoded.resolve(&mut resolver(materialized_bridge())),
        Err(ObjectDefinitionResolutionError::Definition(
            ObjectDefinitionIdentityError::StrongRoleEntityMismatch
        ))
    );

    let wrong_owner_role = RawPlanKey {
        owner: RawPlanOwner::Trusted(strong_owner),
        role: ObjectDefinitionPlanRole::OdrMemberPrimary,
    };
    let decoded =
        decode_canonical::<DecodedObjectDefinitionPlanKey>(&encode(&wrong_owner_role).unwrap())
            .unwrap();
    assert_eq!(
        decoded.resolve(&mut resolver(materialized_bridge())),
        Err(ObjectDefinitionResolutionError::Definition(
            ObjectDefinitionIdentityError::PlanOwnerRoleMismatch
        ))
    );
}

#[test]
fn plan_resolution_rejects_nonmaterializable_bridge_support() {
    let bridge = static_assert_bridge();
    let raw = RawPlanKey {
        owner: RawPlanOwner::Bridge {
            producer: producer(),
            atom: GeneratedBridgeAtomId::from_key(&bridge).unwrap(),
        },
        role: ObjectDefinitionPlanRole::Strong(StrongDefinitionRole::GeneratedBridge),
    };
    let decoded =
        decode_canonical::<DecodedObjectDefinitionPlanKey>(&encode(&raw).unwrap()).unwrap();

    assert_eq!(
        decoded.resolve(&mut resolver(bridge)),
        Err(ObjectDefinitionResolutionError::Definition(
            ObjectDefinitionIdentityError::NonMaterializableBridgeAtom
        ))
    );
}

#[test]
fn atom_resolution_rejects_an_unknown_plan_reference() {
    let key = ObjectDefinitionAtomKey::new(
        ObjectDefinitionPlanId([99; 32]),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    );
    let decoded =
        decode_canonical::<DecodedObjectDefinitionAtomKey>(&encode(&key).unwrap()).unwrap();

    assert_eq!(
        decoded.resolve(&mut resolver(materialized_bridge())),
        Err(ObjectDefinitionResolutionError::Reference(ResolutionError))
    );
}

#[test]
fn object_definition_decoders_reject_unknown_tags() {
    assert_unknown::<DecodedStrongDefinitionEntity>(b"\xa1\x00\x0e", 14);
    assert_unknown::<DecodedObjectDefinitionPlanOwner>(b"\xa1\x00\x03", 3);
    assert_unknown::<StrongDefinitionRole>(b"\x14", 20);
    assert_unknown::<StrongDefinitionRole>(b"\x0a", 10);
    assert_unknown::<ObjectDefinitionPlanRole>(b"\xa1\x00\x03", 3);
    assert_unknown::<DefinitionAtomRole>(b"\x0a", 10);
    assert_unknown::<ConeImageSupportRole>(b"\x0d", 13);
    assert_unknown::<DecodedDefinitionAtomSubkey>(b"\xa1\x00\x0a", 10);
}

fn assert_unknown<T: scoop_wire::WireDecode + std::fmt::Debug>(bytes: &[u8], tag: u64) {
    let error = decode_canonical::<T>(bytes).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag });
}

fn resolver(bridge: GeneratedBridgeAtomKey) -> Resolver {
    Resolver { bridge }
}

fn strong_plan_cases(
    bridge: StrongDefinitionEntity,
) -> Vec<(StrongDefinitionEntity, StrongDefinitionRole)> {
    use StrongDefinitionRole as R;

    vec![
        (
            StrongDefinitionEntity::callable_body(callable_body()),
            R::CallableBody,
        ),
        (
            StrongDefinitionEntity::callable_body(callable_body()),
            R::CallableRegistration,
        ),
        (
            StrongDefinitionEntity::static_storage(static_storage()),
            R::StaticStorage,
        ),
        (
            StrongDefinitionEntity::static_storage(static_storage()),
            R::RootRegistration,
        ),
        (
            StrongDefinitionEntity::immortal_object(immortal_object()),
            R::ImmortalObject,
        ),
        (
            StrongDefinitionEntity::immortal_object(immortal_object()),
            R::ImmortalRegistration,
        ),
        (
            StrongDefinitionEntity::exact_type(exact_type()),
            R::TypeDescriptor,
        ),
        (
            StrongDefinitionEntity::exact_type(exact_type()),
            R::TypeRegistration,
        ),
        (StrongDefinitionEntity::layout(layout()), R::Layout),
        (StrongDefinitionEntity::scan(scan()), R::ScanProgram),
        (
            StrongDefinitionEntity::dispatch_table(dispatch_table()),
            R::DispatchTable,
        ),
        (
            StrongDefinitionEntity::dispatch_slot(dispatch_slot()),
            R::DispatchSlot,
        ),
        (
            StrongDefinitionEntity::initialization_unit(initialization_unit()),
            R::InitializationCell,
        ),
        (
            StrongDefinitionEntity::initialization_unit(initialization_unit()),
            R::InitializationRegistration,
        ),
        (
            StrongDefinitionEntity::safepoint_site(safepoint_site()),
            R::SafepointRegistration,
        ),
        (
            StrongDefinitionEntity::cone_image(producer()),
            R::ImageDescriptor,
        ),
        (bridge, R::GeneratedBridge),
        (
            StrongDefinitionEntity::root_entry(producer()),
            R::RootEntryDescriptor,
        ),
    ]
}

fn atom_cases() -> Vec<(DefinitionAtomRole, DefinitionAtomSubkey)> {
    vec![
        (DefinitionAtomRole::Primary, DefinitionAtomSubkey::Singleton),
        (
            DefinitionAtomRole::Lsda,
            DefinitionAtomSubkey::CallableBody(callable_body()),
        ),
        (
            DefinitionAtomRole::EhFrame,
            DefinitionAtomSubkey::StaticStorage(static_storage()),
        ),
        (
            DefinitionAtomRole::CompactUnwind,
            DefinitionAtomSubkey::ImmortalObject(immortal_object()),
        ),
        (
            DefinitionAtomRole::Stackmap,
            DefinitionAtomSubkey::InitializationUnit(initialization_unit()),
        ),
        (
            DefinitionAtomRole::RuntimeRecord,
            DefinitionAtomSubkey::ExactType(exact_type()),
        ),
        (
            DefinitionAtomRole::AddressTakenConstant,
            DefinitionAtomSubkey::SafepointSite(safepoint_site()),
        ),
        (
            DefinitionAtomRole::Primary,
            DefinitionAtomSubkey::StructuralPath(structural_path()),
        ),
        (
            DefinitionAtomRole::RuntimeRecord,
            DefinitionAtomSubkey::ConeImageSupport(ConeImageSupportRole::Dependencies),
        ),
    ]
}

enum RawPlanOwner {
    Trusted(ObjectDefinitionPlanOwner),
    Bridge {
        producer: ConeIdentity,
        atom: GeneratedBridgeAtomId,
    },
}

impl WireEncode for RawPlanOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Trusted(owner) => owner.encode(encoder),
            Self::Bridge { producer, atom } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                producer.encode(encoder)?;
                encoder.field(2)?;
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(12)?;
                encoder.field(1)?;
                atom.encode(encoder)
            }
        }
    }
}

struct RawPlanKey {
    owner: RawPlanOwner,
    role: ObjectDefinitionPlanRole,
}

impl WireEncode for RawPlanKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}

fn materialized_bridge() -> GeneratedBridgeAtomKey {
    GeneratedBridgeAtomKey::new(
        producer(),
        GeneratedBridgeAtomRoleKey::PrimaryEntry {
            unit: GeneratedBridgeUnitId([14; 32]),
        },
    )
}

fn static_assert_bridge() -> GeneratedBridgeAtomKey {
    GeneratedBridgeAtomKey::new(
        producer(),
        GeneratedBridgeAtomRoleKey::StaticAssertSupport {
            unit: GeneratedBridgeUnitId([14; 32]),
            layout: CanonicalCAbiLayoutFingerprint([15; 32]),
        },
    )
}

fn structural_path() -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, 2),
        [],
    )
}

const fn producer() -> ConeIdentity {
    ConeIdentity([1; 32])
}

const fn callable_body() -> PersistentCallableBodyId {
    PersistentCallableBodyId([2; 32])
}

const fn static_storage() -> PersistentStaticStorageId {
    PersistentStaticStorageId([3; 32])
}

const fn immortal_object() -> PersistentImmortalObjectId {
    PersistentImmortalObjectId([4; 32])
}

const fn exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId([5; 32])
}

const fn layout() -> PersistentLayoutId {
    PersistentLayoutId([6; 32])
}

const fn scan() -> PersistentScanId {
    PersistentScanId([7; 32])
}

const fn dispatch_table() -> PersistentDispatchTableId {
    PersistentDispatchTableId([8; 32])
}

const fn dispatch_slot() -> PersistentDispatchSlotId {
    PersistentDispatchSlotId([9; 32])
}

const fn initialization_unit() -> PersistentInitializationUnitId {
    PersistentInitializationUnitId([10; 32])
}

const fn safepoint_site() -> PersistentSafepointSiteId {
    PersistentSafepointSiteId([11; 32])
}

const fn odr_member() -> OdrMemberId {
    OdrMemberId([12; 32])
}

const fn definition_plan() -> ObjectDefinitionPlanId {
    ObjectDefinitionPlanId([13; 32])
}
