use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode};

use super::{GeneratedBridgeAtomKey, StructuralDefinitionPath};
use crate::ids::derive_persistent_id;
use crate::{
    ConeIdentity, GeneratedBridgeAtomId, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
    OdrMemberId, PersistentCallableBodyId, PersistentDispatchSlotId, PersistentDispatchTableId,
    PersistentExactTypeId, PersistentImmortalObjectId, PersistentInitializationUnitId,
    PersistentLayoutId, PersistentSafepointSiteId, PersistentScanId, PersistentStaticStorageId,
    PersistentSymbolKey,
};

mod decode;

pub use decode::{
    DecodedDefinitionAtomSubkey, DecodedObjectDefinitionAtomKey, DecodedObjectDefinitionPlanKey,
    DecodedObjectDefinitionPlanOwner, DecodedStrongDefinitionEntity, DefinitionAtomResolver,
    ObjectDefinitionResolutionError, StrongDefinitionResolver,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StrongDefinitionEntityKind {
    CallableBody(PersistentCallableBodyId),
    StaticStorage(PersistentStaticStorageId),
    ImmortalObject(PersistentImmortalObjectId),
    ExactType(PersistentExactTypeId),
    Layout(PersistentLayoutId),
    Scan(PersistentScanId),
    DispatchTable(PersistentDispatchTableId),
    DispatchSlot(PersistentDispatchSlotId),
    InitializationUnit(PersistentInitializationUnitId),
    SafepointSite(PersistentSafepointSiteId),
    ConeImage(ConeIdentity),
    GeneratedBridgeAtom(GeneratedBridgeAtomId),
    RootEntry(ConeIdentity),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StrongDefinitionEntity(StrongDefinitionEntityKind);

impl StrongDefinitionEntity {
    pub const fn callable_body(id: PersistentCallableBodyId) -> Self {
        Self(StrongDefinitionEntityKind::CallableBody(id))
    }

    pub const fn static_storage(id: PersistentStaticStorageId) -> Self {
        Self(StrongDefinitionEntityKind::StaticStorage(id))
    }

    pub const fn immortal_object(id: PersistentImmortalObjectId) -> Self {
        Self(StrongDefinitionEntityKind::ImmortalObject(id))
    }

    pub const fn exact_type(id: PersistentExactTypeId) -> Self {
        Self(StrongDefinitionEntityKind::ExactType(id))
    }

    pub const fn layout(id: PersistentLayoutId) -> Self {
        Self(StrongDefinitionEntityKind::Layout(id))
    }

    pub const fn scan(id: PersistentScanId) -> Self {
        Self(StrongDefinitionEntityKind::Scan(id))
    }

    pub const fn dispatch_table(id: PersistentDispatchTableId) -> Self {
        Self(StrongDefinitionEntityKind::DispatchTable(id))
    }

    pub const fn dispatch_slot(id: PersistentDispatchSlotId) -> Self {
        Self(StrongDefinitionEntityKind::DispatchSlot(id))
    }

    pub const fn initialization_unit(id: PersistentInitializationUnitId) -> Self {
        Self(StrongDefinitionEntityKind::InitializationUnit(id))
    }

    pub const fn safepoint_site(id: PersistentSafepointSiteId) -> Self {
        Self(StrongDefinitionEntityKind::SafepointSite(id))
    }

    pub const fn cone_image(id: ConeIdentity) -> Self {
        Self(StrongDefinitionEntityKind::ConeImage(id))
    }

    pub const fn root_entry(id: ConeIdentity) -> Self {
        Self(StrongDefinitionEntityKind::RootEntry(id))
    }

    pub fn generated_bridge_atom(
        key: &GeneratedBridgeAtomKey,
    ) -> Result<Self, ObjectDefinitionIdentityError> {
        if !key.atom().is_materializable() {
            return Err(ObjectDefinitionIdentityError::NonMaterializableBridgeAtom);
        }
        GeneratedBridgeAtomId::from_key(key)
            .map(StrongDefinitionEntityKind::GeneratedBridgeAtom)
            .map(Self)
            .map_err(ObjectDefinitionIdentityError::Hash)
    }

    pub const fn kind(self) -> StrongDefinitionEntityKind {
        self.0
    }
}

impl WireEncode for StrongDefinitionEntity {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.0 {
            StrongDefinitionEntityKind::CallableBody(id) => encode_value_sum(encoder, 1, &id),
            StrongDefinitionEntityKind::StaticStorage(id) => encode_value_sum(encoder, 2, &id),
            StrongDefinitionEntityKind::ImmortalObject(id) => encode_value_sum(encoder, 3, &id),
            StrongDefinitionEntityKind::ExactType(id) => encode_value_sum(encoder, 4, &id),
            StrongDefinitionEntityKind::Layout(id) => encode_value_sum(encoder, 5, &id),
            StrongDefinitionEntityKind::Scan(id) => encode_value_sum(encoder, 6, &id),
            StrongDefinitionEntityKind::DispatchTable(id) => encode_value_sum(encoder, 7, &id),
            StrongDefinitionEntityKind::DispatchSlot(id) => encode_value_sum(encoder, 8, &id),
            StrongDefinitionEntityKind::InitializationUnit(id) => encode_value_sum(encoder, 9, &id),
            StrongDefinitionEntityKind::SafepointSite(id) => encode_value_sum(encoder, 10, &id),
            StrongDefinitionEntityKind::ConeImage(id) => encode_value_sum(encoder, 11, &id),
            StrongDefinitionEntityKind::GeneratedBridgeAtom(id) => {
                encode_value_sum(encoder, 12, &id)
            }
            StrongDefinitionEntityKind::RootEntry(id) => encode_value_sum(encoder, 13, &id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StrongDefinitionRole {
    CallableBody,
    StaticStorage,
    ImmortalObject,
    TypeDescriptor,
    Layout,
    ScanProgram,
    DispatchTable,
    DispatchSlot,
    InitializationCell,
    RootRegistration,
    ImmortalRegistration,
    InitializationRegistration,
    TypeRegistration,
    SafepointRegistration,
    CallableRegistration,
    ImageDescriptor,
    GeneratedBridge,
    RootEntryDescriptor,
}

impl WireEncode for StrongDefinitionRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::CallableBody => 1,
            Self::StaticStorage => 2,
            Self::ImmortalObject => 3,
            Self::TypeDescriptor => 4,
            Self::Layout => 5,
            Self::ScanProgram => 6,
            Self::DispatchTable => 7,
            Self::DispatchSlot => 8,
            Self::InitializationCell => 9,
            Self::RootRegistration => 11,
            Self::ImmortalRegistration => 12,
            Self::InitializationRegistration => 13,
            Self::TypeRegistration => 14,
            Self::SafepointRegistration => 15,
            Self::CallableRegistration => 16,
            Self::ImageDescriptor => 17,
            Self::GeneratedBridge => 18,
            Self::RootEntryDescriptor => 19,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ObjectDefinitionPlanOwner {
    Strong {
        producer: ConeIdentity,
        entity: StrongDefinitionEntity,
    },
    Odr {
        member: OdrMemberId,
    },
}

impl WireEncode for ObjectDefinitionPlanOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Strong { producer, entity } => {
                encoder.map(3)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                producer.encode(encoder)?;
                encoder.field(2)?;
                entity.encode(encoder)
            }
            Self::Odr { member } => encode_value_sum(encoder, 2, member),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ObjectDefinitionPlanRole {
    Strong(StrongDefinitionRole),
    OdrMemberPrimary,
}

impl WireEncode for ObjectDefinitionPlanRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Strong(role) => encode_value_sum(encoder, 1, role),
            Self::OdrMemberPrimary => encode_empty_sum(encoder, 2),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObjectDefinitionPlanKey {
    owner: ObjectDefinitionPlanOwner,
    definition_role: ObjectDefinitionPlanRole,
}

impl ObjectDefinitionPlanKey {
    pub fn strong(
        producer: ConeIdentity,
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    ) -> Result<Self, ObjectDefinitionIdentityError> {
        let key = Self {
            owner: ObjectDefinitionPlanOwner::Strong { producer, entity },
            definition_role: ObjectDefinitionPlanRole::Strong(role),
        };
        if key.primary_symbol_key().is_none() {
            return Err(ObjectDefinitionIdentityError::StrongRoleEntityMismatch);
        }
        Ok(key)
    }

    pub const fn odr(member: OdrMemberId) -> Self {
        Self {
            owner: ObjectDefinitionPlanOwner::Odr { member },
            definition_role: ObjectDefinitionPlanRole::OdrMemberPrimary,
        }
    }

    pub const fn owner(self) -> ObjectDefinitionPlanOwner {
        self.owner
    }

    pub const fn definition_role(self) -> ObjectDefinitionPlanRole {
        self.definition_role
    }

    pub const fn primary_symbol_key(self) -> Option<PersistentSymbolKey> {
        let (
            ObjectDefinitionPlanOwner::Strong { entity, .. },
            ObjectDefinitionPlanRole::Strong(role),
        ) = (self.owner, self.definition_role)
        else {
            return None;
        };
        entity.primary_symbol_key(role)
    }
}

impl StrongDefinitionEntity {
    /// Resolves the symbol role from the entity kind, independently of its provider.
    pub const fn primary_symbol_key(
        self,
        role: StrongDefinitionRole,
    ) -> Option<PersistentSymbolKey> {
        match (self.kind(), role) {
            (StrongDefinitionEntityKind::CallableBody(id), StrongDefinitionRole::CallableBody) => {
                Some(PersistentSymbolKey::CallableBody(id))
            }
            (
                StrongDefinitionEntityKind::StaticStorage(id),
                StrongDefinitionRole::StaticStorage,
            ) => Some(PersistentSymbolKey::StaticStorage(id)),
            (
                StrongDefinitionEntityKind::ImmortalObject(id),
                StrongDefinitionRole::ImmortalObject,
            ) => Some(PersistentSymbolKey::ImmortalObject(id)),
            (StrongDefinitionEntityKind::ExactType(id), StrongDefinitionRole::TypeDescriptor) => {
                Some(PersistentSymbolKey::TypeDescriptor(id))
            }
            (StrongDefinitionEntityKind::Layout(id), StrongDefinitionRole::Layout) => {
                Some(PersistentSymbolKey::Layout(id))
            }
            (StrongDefinitionEntityKind::Scan(id), StrongDefinitionRole::ScanProgram) => {
                Some(PersistentSymbolKey::ScanProgram(id))
            }
            (
                StrongDefinitionEntityKind::DispatchTable(id),
                StrongDefinitionRole::DispatchTable,
            ) => Some(PersistentSymbolKey::DispatchTable(id)),
            (StrongDefinitionEntityKind::DispatchSlot(id), StrongDefinitionRole::DispatchSlot) => {
                Some(PersistentSymbolKey::DispatchSlot(id))
            }
            (
                StrongDefinitionEntityKind::InitializationUnit(id),
                StrongDefinitionRole::InitializationCell,
            ) => Some(PersistentSymbolKey::InitializationCell(id)),
            (
                StrongDefinitionEntityKind::StaticStorage(id),
                StrongDefinitionRole::RootRegistration,
            ) => Some(PersistentSymbolKey::RootRegistration(id)),
            (
                StrongDefinitionEntityKind::ImmortalObject(id),
                StrongDefinitionRole::ImmortalRegistration,
            ) => Some(PersistentSymbolKey::ImmortalRegistration(id)),
            (
                StrongDefinitionEntityKind::InitializationUnit(id),
                StrongDefinitionRole::InitializationRegistration,
            ) => Some(PersistentSymbolKey::InitializationRegistration(id)),
            (StrongDefinitionEntityKind::ExactType(id), StrongDefinitionRole::TypeRegistration) => {
                Some(PersistentSymbolKey::TypeRegistration(id))
            }
            (
                StrongDefinitionEntityKind::SafepointSite(id),
                StrongDefinitionRole::SafepointRegistration,
            ) => Some(PersistentSymbolKey::SafepointRegistration(id)),
            (
                StrongDefinitionEntityKind::CallableBody(id),
                StrongDefinitionRole::CallableRegistration,
            ) => Some(PersistentSymbolKey::CallableRegistration(id)),
            (StrongDefinitionEntityKind::ConeImage(id), StrongDefinitionRole::ImageDescriptor) => {
                Some(PersistentSymbolKey::ImageDescriptor(id))
            }
            (
                StrongDefinitionEntityKind::GeneratedBridgeAtom(id),
                StrongDefinitionRole::GeneratedBridge,
            ) => Some(PersistentSymbolKey::GeneratedBridge(id)),
            (
                StrongDefinitionEntityKind::RootEntry(id),
                StrongDefinitionRole::RootEntryDescriptor,
            ) => Some(PersistentSymbolKey::RootEntryDescriptor(id)),
            _ => None,
        }
    }
}

impl WireEncode for ObjectDefinitionPlanKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.definition_role.encode(encoder)
    }
}

impl ObjectDefinitionPlanId {
    pub fn from_key(key: &ObjectDefinitionPlanKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-object-definition-plan-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefinitionAtomRole {
    ContextKeyCell,
    ContextKeyTable,
    Primary,
    Lsda,
    EhFrame,
    CompactUnwind,
    Stackmap,
    RuntimeRecord,
    AddressTakenConstant,
}

impl WireEncode for DefinitionAtomRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Primary => 1,
            Self::Lsda => 2,
            Self::EhFrame => 3,
            Self::CompactUnwind => 4,
            Self::Stackmap => 5,
            Self::RuntimeRecord => 6,
            Self::AddressTakenConstant => 7,
            Self::ContextKeyCell => 8,
            Self::ContextKeyTable => 9,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ConeImageSupportRole {
    CoordinateGroup,
    CoordinateName,
    CoordinateVersion,
    Dependencies,
    StaticStorages,
    ImmortalObjects,
    InitializationUnits,
    TypeRegistrations,
    Safepoints,
    Callables,
    ArrayBoundsMessage,
    ArraySizeOverflowMessage,
}

impl WireEncode for ConeImageSupportRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::CoordinateGroup => 1,
            Self::CoordinateName => 2,
            Self::CoordinateVersion => 3,
            Self::Dependencies => 4,
            Self::StaticStorages => 5,
            Self::ImmortalObjects => 6,
            Self::InitializationUnits => 7,
            Self::TypeRegistrations => 8,
            Self::Safepoints => 9,
            Self::Callables => 10,
            Self::ArrayBoundsMessage => 11,
            Self::ArraySizeOverflowMessage => 12,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefinitionAtomSubkey {
    Singleton,
    CallableBody(PersistentCallableBodyId),
    StaticStorage(PersistentStaticStorageId),
    ImmortalObject(PersistentImmortalObjectId),
    InitializationUnit(PersistentInitializationUnitId),
    ExactType(PersistentExactTypeId),
    SafepointSite(PersistentSafepointSiteId),
    StructuralPath(StructuralDefinitionPath),
    ConeImageSupport(ConeImageSupportRole),
}

impl WireEncode for DefinitionAtomSubkey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Singleton => encode_empty_sum(encoder, 1),
            Self::CallableBody(id) => encode_value_sum(encoder, 2, id),
            Self::StaticStorage(id) => encode_value_sum(encoder, 3, id),
            Self::ImmortalObject(id) => encode_value_sum(encoder, 4, id),
            Self::InitializationUnit(id) => encode_value_sum(encoder, 5, id),
            Self::ExactType(id) => encode_value_sum(encoder, 6, id),
            Self::SafepointSite(id) => encode_value_sum(encoder, 7, id),
            Self::StructuralPath(path) => encode_value_sum(encoder, 8, path),
            Self::ConeImageSupport(role) => encode_value_sum(encoder, 9, role),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObjectDefinitionAtomKey {
    plan: ObjectDefinitionPlanId,
    role: DefinitionAtomRole,
    subkey: DefinitionAtomSubkey,
}

impl ObjectDefinitionAtomKey {
    pub const fn new(
        plan: ObjectDefinitionPlanId,
        role: DefinitionAtomRole,
        subkey: DefinitionAtomSubkey,
    ) -> Self {
        Self { plan, role, subkey }
    }

    pub const fn plan(&self) -> ObjectDefinitionPlanId {
        self.plan
    }

    pub const fn role(&self) -> DefinitionAtomRole {
        self.role
    }

    pub const fn subkey(&self) -> &DefinitionAtomSubkey {
        &self.subkey
    }
}

impl WireEncode for ObjectDefinitionAtomKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.plan.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        self.subkey.encode(encoder)
    }
}

impl ObjectDefinitionAtomId {
    pub fn from_key(key: &ObjectDefinitionAtomKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-object-definition-atom-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectDefinitionIdentityError {
    StrongRoleEntityMismatch,
    PlanOwnerRoleMismatch,
    NonMaterializableBridgeAtom,
    Hash(HashError),
}

impl fmt::Display for ObjectDefinitionIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StrongRoleEntityMismatch => {
                formatter.write_str("strong definition role does not accept this entity kind")
            }
            Self::PlanOwnerRoleMismatch => {
                formatter.write_str("object definition plan owner does not accept this role")
            }
            Self::NonMaterializableBridgeAtom => {
                formatter.write_str("static-assert bridge support has no materialized object atom")
            }
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ObjectDefinitionIdentityError {}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests {
    use super::{
        ConeImageSupportRole, DefinitionAtomRole, DefinitionAtomSubkey, ObjectDefinitionAtomKey,
        ObjectDefinitionIdentityError, ObjectDefinitionPlanKey, ObjectDefinitionPlanOwner,
        ObjectDefinitionPlanRole, StrongDefinitionEntity, StrongDefinitionRole,
    };
    use crate::{
        CanonicalCAbiLayoutFingerprint, ConeIdentity, GeneratedBridgeAtomId,
        GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey, GeneratedBridgeUnitId,
        ObjectDefinitionAtomId, ObjectDefinitionPlanId, OdrMemberId, PersistentCallableBodyId,
        PersistentDispatchSlotId, PersistentDispatchTableId, PersistentExactTypeId,
        PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentLayoutId,
        PersistentSafepointSiteId, PersistentScanId, PersistentStaticStorageId,
        PersistentSymbolKey,
    };

    const ALL_STRONG_ROLES: [StrongDefinitionRole; 18] = [
        StrongDefinitionRole::CallableBody,
        StrongDefinitionRole::StaticStorage,
        StrongDefinitionRole::ImmortalObject,
        StrongDefinitionRole::TypeDescriptor,
        StrongDefinitionRole::Layout,
        StrongDefinitionRole::ScanProgram,
        StrongDefinitionRole::DispatchTable,
        StrongDefinitionRole::DispatchSlot,
        StrongDefinitionRole::InitializationCell,
        StrongDefinitionRole::RootRegistration,
        StrongDefinitionRole::ImmortalRegistration,
        StrongDefinitionRole::InitializationRegistration,
        StrongDefinitionRole::TypeRegistration,
        StrongDefinitionRole::SafepointRegistration,
        StrongDefinitionRole::CallableRegistration,
        StrongDefinitionRole::ImageDescriptor,
        StrongDefinitionRole::GeneratedBridge,
        StrongDefinitionRole::RootEntryDescriptor,
    ];

    #[test]
    fn strong_plan_closes_the_entity_role_matrix() {
        use StrongDefinitionRole as R;

        let bytes = ConeIdentity::CORE.0;
        let bridge = StrongDefinitionEntity::generated_bridge_atom(&GeneratedBridgeAtomKey::new(
            ConeIdentity::CORE,
            GeneratedBridgeAtomRoleKey::PrimaryEntry {
                unit: GeneratedBridgeUnitId(bytes),
            },
        ))
        .unwrap();
        let cases: [(StrongDefinitionEntity, &[StrongDefinitionRole]); 13] = [
            (
                StrongDefinitionEntity::callable_body(PersistentCallableBodyId(bytes)),
                &[R::CallableBody, R::CallableRegistration],
            ),
            (
                StrongDefinitionEntity::static_storage(PersistentStaticStorageId(bytes)),
                &[R::StaticStorage, R::RootRegistration],
            ),
            (
                StrongDefinitionEntity::immortal_object(PersistentImmortalObjectId(bytes)),
                &[R::ImmortalObject, R::ImmortalRegistration],
            ),
            (
                StrongDefinitionEntity::exact_type(PersistentExactTypeId(bytes)),
                &[R::TypeDescriptor, R::TypeRegistration],
            ),
            (
                StrongDefinitionEntity::layout(PersistentLayoutId(bytes)),
                &[R::Layout],
            ),
            (
                StrongDefinitionEntity::scan(PersistentScanId(bytes)),
                &[R::ScanProgram],
            ),
            (
                StrongDefinitionEntity::dispatch_table(PersistentDispatchTableId(bytes)),
                &[R::DispatchTable],
            ),
            (
                StrongDefinitionEntity::dispatch_slot(PersistentDispatchSlotId(bytes)),
                &[R::DispatchSlot],
            ),
            (
                StrongDefinitionEntity::initialization_unit(PersistentInitializationUnitId(bytes)),
                &[R::InitializationCell, R::InitializationRegistration],
            ),
            (
                StrongDefinitionEntity::safepoint_site(PersistentSafepointSiteId(bytes)),
                &[R::SafepointRegistration],
            ),
            (
                StrongDefinitionEntity::cone_image(ConeIdentity::CORE),
                &[R::ImageDescriptor],
            ),
            (bridge, &[R::GeneratedBridge]),
            (
                StrongDefinitionEntity::root_entry(ConeIdentity::CORE),
                &[R::RootEntryDescriptor],
            ),
        ];

        for (entity, accepted_roles) in cases {
            for role in ALL_STRONG_ROLES {
                assert_eq!(
                    ObjectDefinitionPlanKey::strong(ConeIdentity::CORE, entity, role).is_ok(),
                    accepted_roles.contains(&role),
                    "unexpected matrix result for {entity:?} and {role:?}"
                );
            }
        }

        let key = ObjectDefinitionPlanKey::strong(
            ConeIdentity::CORE,
            StrongDefinitionEntity::callable_body(PersistentCallableBodyId(bytes)),
            StrongDefinitionRole::CallableRegistration,
        )
        .unwrap();
        assert_eq!(
            ObjectDefinitionPlanId::from_key(&key).unwrap().to_string(),
            "de5cfc42126d444d593ff1528569def41ae63afc763c40d270d12ae6129fb16a"
        );
    }

    #[test]
    fn odr_plan_has_only_the_primary_role() {
        let member = OdrMemberId(ConeIdentity::CORE.0);
        let key = ObjectDefinitionPlanKey::odr(member);

        assert_eq!(key.owner(), ObjectDefinitionPlanOwner::Odr { member });
        assert_eq!(
            key.definition_role(),
            ObjectDefinitionPlanRole::OdrMemberPrimary
        );
        assert_eq!(key.primary_symbol_key(), None);
    }

    #[test]
    fn every_strong_role_has_one_exact_primary_symbol_kind() {
        use StrongDefinitionRole as R;

        let bytes = ConeIdentity::CORE.0;
        let callable = PersistentCallableBodyId(bytes);
        let storage = PersistentStaticStorageId(bytes);
        let immortal = PersistentImmortalObjectId(bytes);
        let exact = PersistentExactTypeId(bytes);
        let layout = PersistentLayoutId(bytes);
        let scan = PersistentScanId(bytes);
        let table = PersistentDispatchTableId(bytes);
        let slot = PersistentDispatchSlotId(bytes);
        let initialization = PersistentInitializationUnitId(bytes);
        let safepoint = PersistentSafepointSiteId(bytes);
        let bridge_key = GeneratedBridgeAtomKey::new(
            ConeIdentity::CORE,
            GeneratedBridgeAtomRoleKey::PrimaryEntry {
                unit: GeneratedBridgeUnitId(bytes),
            },
        );
        let bridge = StrongDefinitionEntity::generated_bridge_atom(&bridge_key).unwrap();
        let bridge_id = GeneratedBridgeAtomId::from_key(&bridge_key).unwrap();
        let cases = [
            (
                StrongDefinitionEntity::callable_body(callable),
                R::CallableBody,
                PersistentSymbolKey::CallableBody(callable),
            ),
            (
                StrongDefinitionEntity::static_storage(storage),
                R::StaticStorage,
                PersistentSymbolKey::StaticStorage(storage),
            ),
            (
                StrongDefinitionEntity::immortal_object(immortal),
                R::ImmortalObject,
                PersistentSymbolKey::ImmortalObject(immortal),
            ),
            (
                StrongDefinitionEntity::exact_type(exact),
                R::TypeDescriptor,
                PersistentSymbolKey::TypeDescriptor(exact),
            ),
            (
                StrongDefinitionEntity::layout(layout),
                R::Layout,
                PersistentSymbolKey::Layout(layout),
            ),
            (
                StrongDefinitionEntity::scan(scan),
                R::ScanProgram,
                PersistentSymbolKey::ScanProgram(scan),
            ),
            (
                StrongDefinitionEntity::dispatch_table(table),
                R::DispatchTable,
                PersistentSymbolKey::DispatchTable(table),
            ),
            (
                StrongDefinitionEntity::dispatch_slot(slot),
                R::DispatchSlot,
                PersistentSymbolKey::DispatchSlot(slot),
            ),
            (
                StrongDefinitionEntity::initialization_unit(initialization),
                R::InitializationCell,
                PersistentSymbolKey::InitializationCell(initialization),
            ),
            (
                StrongDefinitionEntity::static_storage(storage),
                R::RootRegistration,
                PersistentSymbolKey::RootRegistration(storage),
            ),
            (
                StrongDefinitionEntity::immortal_object(immortal),
                R::ImmortalRegistration,
                PersistentSymbolKey::ImmortalRegistration(immortal),
            ),
            (
                StrongDefinitionEntity::initialization_unit(initialization),
                R::InitializationRegistration,
                PersistentSymbolKey::InitializationRegistration(initialization),
            ),
            (
                StrongDefinitionEntity::exact_type(exact),
                R::TypeRegistration,
                PersistentSymbolKey::TypeRegistration(exact),
            ),
            (
                StrongDefinitionEntity::safepoint_site(safepoint),
                R::SafepointRegistration,
                PersistentSymbolKey::SafepointRegistration(safepoint),
            ),
            (
                StrongDefinitionEntity::callable_body(callable),
                R::CallableRegistration,
                PersistentSymbolKey::CallableRegistration(callable),
            ),
            (
                StrongDefinitionEntity::cone_image(ConeIdentity::CORE),
                R::ImageDescriptor,
                PersistentSymbolKey::ImageDescriptor(ConeIdentity::CORE),
            ),
            (
                bridge,
                R::GeneratedBridge,
                PersistentSymbolKey::GeneratedBridge(bridge_id),
            ),
            (
                StrongDefinitionEntity::root_entry(ConeIdentity::CORE),
                R::RootEntryDescriptor,
                PersistentSymbolKey::RootEntryDescriptor(ConeIdentity::CORE),
            ),
        ];
        for (entity, role, expected) in cases {
            let key = ObjectDefinitionPlanKey::strong(ConeIdentity::CORE, entity, role).unwrap();
            assert_eq!(key.primary_symbol_key(), Some(expected));
        }
    }

    #[test]
    fn static_assert_bridge_support_cannot_become_a_definition_plan() {
        let key = GeneratedBridgeAtomKey::new(
            ConeIdentity::CORE,
            GeneratedBridgeAtomRoleKey::StaticAssertSupport {
                unit: GeneratedBridgeUnitId(ConeIdentity::CORE.0),
                layout: CanonicalCAbiLayoutFingerprint(ConeIdentity::CORE.0),
            },
        );
        assert_eq!(
            StrongDefinitionEntity::generated_bridge_atom(&key),
            Err(ObjectDefinitionIdentityError::NonMaterializableBridgeAtom)
        );
    }

    #[test]
    fn definition_atom_has_a_fixed_typed_identity() {
        let plan = ObjectDefinitionPlanId(ConeIdentity::CORE.0);
        let storage = PersistentStaticStorageId(ConeIdentity::SINGLE_FILE.0);
        let key = ObjectDefinitionAtomKey::new(
            plan,
            DefinitionAtomRole::RuntimeRecord,
            DefinitionAtomSubkey::StaticStorage(storage),
        );
        assert_eq!(
            ObjectDefinitionAtomId::from_key(&key).unwrap().to_string(),
            "42549de58fb776033ef63dc927231abe1a1d5e6e602fdaf651c90be5c69745ca"
        );
        assert_eq!(key.plan(), plan);
        assert_eq!(key.role(), DefinitionAtomRole::RuntimeRecord);
        assert_eq!(key.subkey(), &DefinitionAtomSubkey::StaticStorage(storage));
    }

    #[test]
    fn cone_image_support_atom_has_a_fixed_typed_identity() {
        let plan = ObjectDefinitionPlanId(ConeIdentity::CORE.0);
        let key = ObjectDefinitionAtomKey::new(
            plan,
            DefinitionAtomRole::RuntimeRecord,
            DefinitionAtomSubkey::ConeImageSupport(ConeImageSupportRole::Dependencies),
        );
        assert_eq!(key.plan(), plan);
        assert_eq!(key.role(), DefinitionAtomRole::RuntimeRecord);
        assert_eq!(
            key.subkey(),
            &DefinitionAtomSubkey::ConeImageSupport(ConeImageSupportRole::Dependencies)
        );
    }
}
