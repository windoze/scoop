use scoop_wire::{DecodeLimits, WireEncode, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedImmortalObjectKey, DecodedLayoutKey, DecodedScanKey, DecodedStaticStorageKey,
    LayoutKeyResolutionError, StaticStorageResolutionError,
};
use crate::{
    CallableMaterialization, CallableMaterializationContext, CallableOwner, CallableTemplateOwner,
    CapabilityId, ConeIdentity, DefinitionOwner, ImmortalObjectKey, ImmortalObjectOwner,
    InitializationUnitKey, LayoutKey, MainCallableBodyId, NonEmptyVec,
    PersistentCallableApplicationId, PersistentCallableBodyId, PersistentConstructorId,
    PersistentExactTypeId, PersistentExtensionPropertyId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentIdMismatch,
    PersistentIdResolver, PersistentInitializationUnitId, PersistentKeyResolver,
    PersistentLayoutId, PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId,
    PropertyOwner, RepresentationRole, RuntimeIdentityError, ScanKey, ScanRole, StaticStorageKey,
    StorageRole, StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

struct Resolver;

macro_rules! id_resolver {
    ($id:ty, $expected:expr) => {
        impl PersistentIdResolver<$id> for Resolver {
            type Error = ResolutionError;

            fn resolve(&mut self, id: crate::DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                id.verify($expected)
                    .map_err(|_: PersistentIdMismatch<$id>| ResolutionError)
            }
        }
    };
}

id_resolver!(ConeIdentity, cone());
id_resolver!(PersistentExactTypeId, exact_type());
id_resolver!(PersistentLayoutId, layout_id());
id_resolver!(PersistentTypeId, nominal_type());
id_resolver!(PersistentPropertyId, property());
id_resolver!(PersistentExtensionPropertyId, extension_property());
id_resolver!(PersistentFunctionId, function());
id_resolver!(PersistentGenericFunctionId, generic_function());
id_resolver!(PersistentConstructorId, constructor());
id_resolver!(PersistentPropertyAccessorId, accessor());
id_resolver!(PersistentGeneratedCallableId, generated_callable());
id_resolver!(PersistentCallableApplicationId, application());
id_resolver!(PersistentCallableBodyId, callable_body());

impl PersistentIdResolver<PersistentInitializationUnitId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<PersistentInitializationUnitId>,
    ) -> Result<PersistentInitializationUnitId, Self::Error> {
        id.verify(initialization_unit_id())
            .or_else(|_| id.verify(delegated_unit_id()))
            .map_err(|_: PersistentIdMismatch<PersistentInitializationUnitId>| ResolutionError)
    }
}

impl PersistentKeyResolver<PersistentInitializationUnitId, InitializationUnitKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: crate::DecodedPersistentId<PersistentInitializationUnitId>,
    ) -> Result<InitializationUnitKey, Self::Error> {
        let delegated = delegated_unit();
        if id.verify(delegated_unit_id()).is_ok() {
            return Ok(delegated);
        }
        id.verify(initialization_unit_id())
            .map(|_| initialization_unit())
            .map_err(|_: PersistentIdMismatch<PersistentInitializationUnitId>| ResolutionError)
    }
}

#[test]
fn every_layout_and_scan_role_round_trips_and_resolves() {
    for representation in [
        RepresentationRole::ManagedValue,
        RepresentationRole::ManagedObject,
        RepresentationRole::CValue,
        RepresentationRole::NativeFunctionPointer,
    ] {
        let key = LayoutKey::darwin_aarch64(exact_type(), representation);
        let decoded =
            decode_canonical::<DecodedLayoutKey>(&encode(&key).unwrap(), DecodeLimits::default())
                .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), key);
    }

    for role in [
        ScanRole::InlineValue,
        ScanRole::ManagedObject,
        ScanRole::ArrayElement,
    ] {
        let key = ScanKey::new(layout_id(), role);
        let decoded =
            decode_canonical::<DecodedScanKey>(&encode(&key).unwrap(), DecodeLimits::default())
                .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), key);
    }
}

#[test]
fn layout_resolution_rejects_an_unregistered_target_profile() {
    let raw = RawLayoutKey {
        exact_type: exact_type(),
        target_profile: CapabilityId::new("org.scoop-lang.target-profile", "darwin-xarch64", 1)
            .unwrap(),
        representation: RepresentationRole::ManagedValue,
    };
    let decoded =
        decode_canonical::<DecodedLayoutKey>(&encode(&raw).unwrap(), DecodeLimits::default())
            .unwrap();

    assert!(matches!(
        decoded.resolve(&mut Resolver),
        Err(LayoutKeyResolutionError::TargetProfile(_))
    ));
}

#[test]
fn every_structurally_valid_static_storage_shape_round_trips() {
    let delegated = delegated_unit();
    let main = MainCallableBodyId::from_body(callable_body());
    let values = vec![
        StaticStorageKey::property_backing(property_owner()),
        StaticStorageKey::property_delegate(property_owner()),
        StaticStorageKey::static_place_for_property(property_owner()),
        StaticStorageKey::singleton_published_root(nominal_type()),
        StaticStorageKey::initialization_failure_root(initialization_unit_id()),
        StaticStorageKey::root_entry_failure_root(cone(), main),
        StaticStorageKey::delegated_application_backing(&delegated).unwrap(),
        StaticStorageKey::delegated_application_delegate(&delegated).unwrap(),
        StaticStorageKey::static_place_for_delegated_application(&delegated).unwrap(),
    ];

    for value in values {
        let decoded = decode_canonical::<DecodedStaticStorageKey>(
            &encode(&value).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), value);
    }
}

#[test]
fn static_storage_resolution_rechecks_the_owner_role_matrix() {
    let invalid_owner = RawStaticStorageKey {
        owner: DefinitionOwner::Callable(CallableOwner::Function(function())),
        role: StorageRole::PropertyBacking,
    };
    let decoded = decode_canonical::<DecodedStaticStorageKey>(
        &encode(&invalid_owner).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(StaticStorageResolutionError::InvalidOwnerRole(
            StorageRole::PropertyBacking
        ))
    );

    let wrong_unit = RawStaticStorageKey {
        owner: DefinitionOwner::InitializationUnit(initialization_unit_id()),
        role: StorageRole::PropertyDelegate,
    };
    let decoded = decode_canonical::<DecodedStaticStorageKey>(
        &encode(&wrong_unit).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(StaticStorageResolutionError::Shape(
            RuntimeIdentityError::ExpectedDelegatedApplication
        ))
    );
}

#[test]
fn every_immortal_object_owner_round_trips_and_resolves() {
    let owners = [
        ImmortalObjectOwner::Callable(CallableMaterialization::new(
            CallableTemplateOwner::Function(function()),
            CallableMaterializationContext::NoSubstitution,
        )),
        ImmortalObjectOwner::Property(property_owner()),
        ImmortalObjectOwner::InitializationUnit(initialization_unit_id()),
    ];

    for owner in owners {
        let value = ImmortalObjectKey::string_constant(owner, string_path());
        let decoded = decode_canonical::<DecodedImmortalObjectKey>(
            &encode(&value).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), value);
    }
}

#[test]
fn runtime_identity_roles_reject_unknown_tags() {
    assert_unknown::<RepresentationRole>(b"\x05", 5);
    assert_unknown::<ScanRole>(b"\x04", 4);
    assert_unknown::<StorageRole>(b"\x07", 7);
    assert_unknown::<crate::ImmortalObjectRole>(b"\x02", 2);
}

fn assert_unknown<T: scoop_wire::WireDecode + std::fmt::Debug>(bytes: &[u8], tag: u64) {
    let error = decode_canonical::<T>(bytes, DecodeLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag });
}

fn property_owner() -> PropertyOwner {
    PropertyOwner::Property(property())
}

fn initialization_unit() -> InitializationUnitKey {
    InitializationUnitKey::TopLevelProperty(property())
}

fn initialization_unit_id() -> PersistentInitializationUnitId {
    PersistentInitializationUnitId::from_key(&initialization_unit()).unwrap()
}

fn delegated_unit() -> InitializationUnitKey {
    InitializationUnitKey::GenericDelegatedExtensionApplication {
        property: extension_property(),
        receiver_arguments: NonEmptyVec::from_first(exact_type(), []),
    }
}

fn delegated_unit_id() -> PersistentInitializationUnitId {
    PersistentInitializationUnitId::from_key(&delegated_unit()).unwrap()
}

fn string_path() -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, 0),
        [],
    )
}

const fn cone() -> ConeIdentity {
    ConeIdentity([1; 32])
}

const fn exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId([2; 32])
}

const fn layout_id() -> PersistentLayoutId {
    PersistentLayoutId([3; 32])
}

const fn nominal_type() -> PersistentTypeId {
    PersistentTypeId([4; 32])
}

const fn property() -> PersistentPropertyId {
    PersistentPropertyId([5; 32])
}

const fn extension_property() -> PersistentExtensionPropertyId {
    PersistentExtensionPropertyId([6; 32])
}

const fn function() -> PersistentFunctionId {
    PersistentFunctionId([7; 32])
}

const fn generic_function() -> PersistentGenericFunctionId {
    PersistentGenericFunctionId([8; 32])
}

const fn constructor() -> PersistentConstructorId {
    PersistentConstructorId([9; 32])
}

const fn accessor() -> PersistentPropertyAccessorId {
    PersistentPropertyAccessorId([10; 32])
}

const fn generated_callable() -> PersistentGeneratedCallableId {
    PersistentGeneratedCallableId([11; 32])
}

const fn application() -> PersistentCallableApplicationId {
    PersistentCallableApplicationId([12; 32])
}

const fn callable_body() -> PersistentCallableBodyId {
    PersistentCallableBodyId([13; 32])
}

struct RawLayoutKey {
    exact_type: PersistentExactTypeId,
    target_profile: CapabilityId,
    representation: RepresentationRole,
}

impl WireEncode for RawLayoutKey {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.exact_type.encode(encoder)?;
        encoder.field(2)?;
        self.target_profile.encode(encoder)?;
        encoder.field(3)?;
        self.representation.encode(encoder)
    }
}

struct RawStaticStorageKey {
    owner: DefinitionOwner,
    role: StorageRole,
}

impl WireEncode for RawStaticStorageKey {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}
