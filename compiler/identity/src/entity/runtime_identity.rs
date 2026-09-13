use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode};

use super::{
    CallableMaterialization, CallableOwner, InitializationUnitKey, MainCallableBodyId,
    NominalDeclarationOwner, NominalOwner, PropertyOwner, StructuralDefinitionPath,
};
use crate::ids::derive_persistent_id;
use crate::{
    ConeIdentity, PersistentEnumVariantId, PersistentExactTypeId, PersistentFieldId,
    PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentLayoutId,
    PersistentScanId, PersistentStaticStorageId, PersistentTypeId, TargetProfileWireId,
};

mod decode;

pub use decode::{
    DecodedImmortalObjectKey, DecodedLayoutKey, DecodedScanKey, DecodedStaticStorageKey,
    LayoutKeyResolutionError, StaticStorageResolutionError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RepresentationRole {
    ManagedValue,
    ManagedObject,
    CValue,
    NativeFunctionPointer,
}

impl WireEncode for RepresentationRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::ManagedValue => 1,
            Self::ManagedObject => 2,
            Self::CValue => 3,
            Self::NativeFunctionPointer => 4,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LayoutKey {
    exact_type: PersistentExactTypeId,
    target_profile: TargetProfileWireId,
    representation: RepresentationRole,
}

impl LayoutKey {
    pub const fn new(
        exact_type: PersistentExactTypeId,
        target_profile: TargetProfileWireId,
        representation: RepresentationRole,
    ) -> Self {
        Self {
            exact_type,
            target_profile,
            representation,
        }
    }

    pub fn darwin_aarch64(
        exact_type: PersistentExactTypeId,
        representation: RepresentationRole,
    ) -> Self {
        Self::new(
            exact_type,
            TargetProfileWireId::darwin_aarch64(),
            representation,
        )
    }

    pub const fn exact_type(&self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn representation(&self) -> RepresentationRole {
        self.representation
    }
}

impl WireEncode for LayoutKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.exact_type.encode(encoder)?;
        encoder.field(2)?;
        self.target_profile.encode(encoder)?;
        encoder.field(3)?;
        self.representation.encode(encoder)
    }
}

impl PersistentLayoutId {
    pub fn from_key(key: &LayoutKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-layout-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScanRole {
    InlineValue,
    ManagedObject,
    ArrayElement,
}

impl WireEncode for ScanRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::InlineValue => 1,
            Self::ManagedObject => 2,
            Self::ArrayElement => 3,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScanKey {
    layout: PersistentLayoutId,
    role: ScanRole,
}

impl ScanKey {
    pub const fn new(layout: PersistentLayoutId, role: ScanRole) -> Self {
        Self { layout, role }
    }

    pub const fn layout(self) -> PersistentLayoutId {
        self.layout
    }

    pub const fn role(self) -> ScanRole {
        self.role
    }
}

impl WireEncode for ScanKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.layout.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}

impl PersistentScanId {
    pub fn from_key(key: &ScanKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-scan-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefinitionOwner {
    Nominal(NominalOwner),
    Callable(CallableOwner),
    Property(PropertyOwner),
    Field(PersistentFieldId),
    EnumVariant(PersistentEnumVariantId),
    InitializationUnit(PersistentInitializationUnitId),
    RootEntry {
        root_cone: ConeIdentity,
        main: MainCallableBodyId,
    },
}

impl WireEncode for DefinitionOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Nominal(owner) => encode_value_sum(encoder, 1, owner),
            Self::Callable(owner) => encode_value_sum(encoder, 2, owner),
            Self::Property(owner) => encode_value_sum(encoder, 3, owner),
            Self::Field(id) => encode_value_sum(encoder, 4, id),
            Self::EnumVariant(id) => encode_value_sum(encoder, 5, id),
            Self::InitializationUnit(id) => encode_value_sum(encoder, 6, id),
            Self::RootEntry { root_cone, main } => {
                encoder.map(3)?;
                encode_tag(encoder, 7)?;
                encoder.field(1)?;
                root_cone.encode(encoder)?;
                encoder.field(2)?;
                main.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StorageRole {
    PropertyBacking,
    PropertyDelegate,
    SingletonPublishedRoot,
    InitializationFailureRoot,
    RootEntryFailureRoot,
    StaticPlaceToken,
}

impl WireEncode for StorageRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::PropertyBacking => 1,
            Self::PropertyDelegate => 2,
            Self::SingletonPublishedRoot => 3,
            Self::InitializationFailureRoot => 4,
            Self::RootEntryFailureRoot => 5,
            Self::StaticPlaceToken => 6,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StaticStorageKey {
    owner: DefinitionOwner,
    role: StorageRole,
}

impl StaticStorageKey {
    pub const fn property_backing(owner: PropertyOwner) -> Self {
        Self::new(
            DefinitionOwner::Property(owner),
            StorageRole::PropertyBacking,
        )
    }

    pub const fn property_delegate(owner: PropertyOwner) -> Self {
        Self::new(
            DefinitionOwner::Property(owner),
            StorageRole::PropertyDelegate,
        )
    }

    pub fn delegated_application_backing(
        unit: &InitializationUnitKey,
    ) -> Result<Self, RuntimeIdentityError> {
        Self::delegated_application(unit, StorageRole::PropertyBacking)
    }

    pub fn delegated_application_delegate(
        unit: &InitializationUnitKey,
    ) -> Result<Self, RuntimeIdentityError> {
        Self::delegated_application(unit, StorageRole::PropertyDelegate)
    }

    pub const fn singleton_published_root(owner: PersistentTypeId) -> Self {
        Self::new(
            DefinitionOwner::Nominal(NominalOwner::Declaration(
                NominalDeclarationOwner::Concrete(owner),
            )),
            StorageRole::SingletonPublishedRoot,
        )
    }

    pub const fn initialization_failure_root(unit: PersistentInitializationUnitId) -> Self {
        Self::new(
            DefinitionOwner::InitializationUnit(unit),
            StorageRole::InitializationFailureRoot,
        )
    }

    pub const fn root_entry_failure_root(
        root_cone: ConeIdentity,
        main: MainCallableBodyId,
    ) -> Self {
        Self::new(
            DefinitionOwner::RootEntry { root_cone, main },
            StorageRole::RootEntryFailureRoot,
        )
    }

    pub const fn static_place_for_property(owner: PropertyOwner) -> Self {
        Self::new(
            DefinitionOwner::Property(owner),
            StorageRole::StaticPlaceToken,
        )
    }

    pub fn static_place_for_delegated_application(
        unit: &InitializationUnitKey,
    ) -> Result<Self, RuntimeIdentityError> {
        Self::delegated_application(unit, StorageRole::StaticPlaceToken)
    }

    pub const fn owner(self) -> DefinitionOwner {
        self.owner
    }

    pub const fn role(self) -> StorageRole {
        self.role
    }

    const fn new(owner: DefinitionOwner, role: StorageRole) -> Self {
        Self { owner, role }
    }

    fn delegated_application(
        unit: &InitializationUnitKey,
        role: StorageRole,
    ) -> Result<Self, RuntimeIdentityError> {
        if !matches!(
            unit,
            InitializationUnitKey::GenericDelegatedExtensionApplication { .. }
        ) {
            return Err(RuntimeIdentityError::ExpectedDelegatedApplication);
        }
        let id =
            PersistentInitializationUnitId::from_key(unit).map_err(RuntimeIdentityError::Hash)?;
        Ok(Self::new(DefinitionOwner::InitializationUnit(id), role))
    }
}

impl WireEncode for StaticStorageKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}

impl PersistentStaticStorageId {
    pub fn from_key(key: &StaticStorageKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-static-storage-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ImmortalObjectOwner {
    Callable(CallableMaterialization),
    Property(PropertyOwner),
    InitializationUnit(PersistentInitializationUnitId),
}

impl WireEncode for ImmortalObjectOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Callable(owner) => encode_value_sum(encoder, 1, owner),
            Self::Property(owner) => encode_value_sum(encoder, 2, owner),
            Self::InitializationUnit(id) => encode_value_sum(encoder, 3, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ImmortalObjectRole {
    StringConstant,
}

impl WireEncode for ImmortalObjectRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(1)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImmortalObjectKey {
    owner: ImmortalObjectOwner,
    role: ImmortalObjectRole,
    path: StructuralDefinitionPath,
}

impl ImmortalObjectKey {
    pub const fn string_constant(
        owner: ImmortalObjectOwner,
        path: StructuralDefinitionPath,
    ) -> Self {
        Self {
            owner,
            role: ImmortalObjectRole::StringConstant,
            path,
        }
    }

    pub const fn owner(&self) -> ImmortalObjectOwner {
        self.owner
    }
}

impl WireEncode for ImmortalObjectKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        self.path.encode(encoder)
    }
}

impl PersistentImmortalObjectId {
    pub fn from_key(key: &ImmortalObjectKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-immortal-object-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeIdentityError {
    ExpectedDelegatedApplication,
    Hash(HashError),
}

impl fmt::Display for RuntimeIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedDelegatedApplication => formatter.write_str(
                "storage identity requires a generic delegated extension application unit",
            ),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RuntimeIdentityError {}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
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
        ImmortalObjectKey, ImmortalObjectOwner, LayoutKey, RepresentationRole,
        RuntimeIdentityError, ScanKey, ScanRole, StaticStorageKey,
    };
    use crate::{
        ConeIdentity, InitializationUnitKey, NonEmptyVec, PersistentExactTypeId,
        PersistentExtensionPropertyId, PersistentImmortalObjectId, PersistentLayoutId,
        PersistentPropertyId, PersistentScanId, PersistentStaticStorageId, PropertyOwner,
        StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
    };

    #[test]
    fn layout_and_scan_identities_have_fixed_hashes() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let layout_key = LayoutKey::darwin_aarch64(exact, RepresentationRole::ManagedObject);
        let layout = PersistentLayoutId::from_key(&layout_key).unwrap();
        let scan_key = ScanKey::new(layout, ScanRole::ManagedObject);

        assert_eq!(
            layout.to_string(),
            "bb06e9d11e25e1fc242e7d0396404a0bb8e1505641ed5bc0986df03fe697544a"
        );
        assert_eq!(
            PersistentScanId::from_key(&scan_key).unwrap().to_string(),
            "bb2bf6c737202b3a2daea82cf34e784ae5a1cfebb149a014cf46bbe9edf041aa"
        );
    }

    #[test]
    fn delegated_storage_requires_the_application_unit_variant() {
        let ordinary =
            InitializationUnitKey::TopLevelProperty(PersistentPropertyId(ConeIdentity::CORE.0));
        assert_eq!(
            StaticStorageKey::delegated_application_backing(&ordinary),
            Err(RuntimeIdentityError::ExpectedDelegatedApplication)
        );

        let delegated = InitializationUnitKey::GenericDelegatedExtensionApplication {
            property: PersistentExtensionPropertyId(ConeIdentity::CORE.0),
            receiver_arguments: NonEmptyVec::from_first(
                PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0),
                [],
            ),
        };
        let key = StaticStorageKey::delegated_application_backing(&delegated).unwrap();
        assert_eq!(
            PersistentStaticStorageId::from_key(&key)
                .unwrap()
                .to_string(),
            "7f77215e992c510d7ab24f357cb5f9276371edc9d782242240b82e6b83b7eda0"
        );
    }

    #[test]
    fn string_constant_identity_keeps_typed_owner_and_structural_path() {
        let owner = ImmortalObjectOwner::Property(PropertyOwner::Property(PersistentPropertyId(
            ConeIdentity::CORE.0,
        )));
        let path = StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, 0),
            [],
        );
        let key = ImmortalObjectKey::string_constant(owner, path);
        assert_eq!(
            PersistentImmortalObjectId::from_key(&key)
                .unwrap()
                .to_string(),
            "739dfeca9079796f0bd48e38141042b0fb695865f49d93a973f4058a210f8504"
        );
    }
}
