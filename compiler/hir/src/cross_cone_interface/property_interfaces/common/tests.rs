use std::fmt;

use scoop_identity::{
    AccessorRole, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DecodedPersistentId, DefinitionOwnerChain, PackagePath, PersistentIdResolver,
    PersistentPropertyAccessorId, PersistentPropertyId, PropertyAccessorKey, PropertyOwner,
    SourceDeclarationKey, SourceDeclarationSite,
};
use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn property_leaf_enums_have_fixed_wire() {
    assert_eq!(
        encode(&PropertySetterPublicAccessV1::Restricted).unwrap(),
        [1]
    );
    assert_eq!(encode(&PropertySetterPublicAccessV1::Public).unwrap(), [2]);
    assert_eq!(encode(&PropertyRepresentationV1::Const).unwrap(), [1]);
    assert_eq!(
        encode(&PropertyRepresentationV1::RuntimeAccessor).unwrap(),
        [2]
    );
    assert_eq!(
        encode(&PropertyRepresentationV1::AbstractSlot).unwrap(),
        [3]
    );
    assert_eq!(encode(&PropertyPublicAccessV1::DirectOnly).unwrap(), [1]);
    assert_eq!(encode(&PropertyPublicAccessV1::PublicSlot).unwrap(), [2]);

    assert_eq!(
        decode_canonical::<PropertyRepresentationV1>(&[2], DecodeLimits::default()).unwrap(),
        PropertyRepresentationV1::RuntimeAccessor
    );
    let error =
        decode_canonical::<PropertyPublicAccessV1>(&[3], DecodeLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}

#[test]
fn property_capability_has_fixed_closed_sum_wire() {
    let (getter, setter) = accessors();
    let read_only = PropertyCapabilityV1::read_only(getter);
    let read_write =
        PropertyCapabilityV1::try_read_write(getter, setter, PropertySetterPublicAccessV1::Public)
            .unwrap();

    assert_eq!(
        encode(&read_only).unwrap(),
        capability_wire(1, getter, None)
    );
    assert_eq!(
        encode(&read_write).unwrap(),
        capability_wire(2, getter, Some((setter, 2)))
    );
    assert_eq!(read_only.getter(), getter);
    assert_eq!(read_only.setter(), None);
    assert_eq!(read_only.setter_access(), None);
    assert!(read_only.is_read_only());
    assert_eq!(read_write.setter(), Some(setter));
    assert_eq!(
        read_write.setter_access(),
        Some(PropertySetterPublicAccessV1::Public)
    );
    assert!(!read_write.is_read_only());
}

#[test]
fn decoded_capability_resolves_each_typed_accessor() {
    let (getter, setter) = accessors();
    let expected = PropertyCapabilityV1::try_read_write(
        getter,
        setter,
        PropertySetterPublicAccessV1::Restricted,
    )
    .unwrap();
    let decoded: DecodedPropertyCapabilityV1 =
        decode_canonical(&encode(&expected).unwrap(), DecodeLimits::default()).unwrap();
    let mut resolver = AccessorResolver { getter, setter };

    assert_eq!(decoded.resolve(&mut resolver).unwrap(), expected);
}

#[test]
fn capability_rejects_duplicate_accessors_and_bad_sum_lengths() {
    let (getter, _) = accessors();
    assert_eq!(
        PropertyCapabilityV1::try_read_write(getter, getter, PropertySetterPublicAccessV1::Public,),
        Err(PropertyCapabilityBuildError::DuplicateAccessor(getter))
    );

    let duplicate_wire = capability_wire(2, getter, Some((getter, 2)));
    let decoded: DecodedPropertyCapabilityV1 =
        decode_canonical(&duplicate_wire, DecodeLimits::default()).unwrap();
    let mut resolver = AccessorResolver {
        getter,
        setter: getter,
    };
    assert_eq!(
        decoded.resolve(&mut resolver),
        Err(PropertyCapabilityResolutionError::Capability(
            PropertyCapabilityBuildError::DuplicateAccessor(getter)
        ))
    );

    let error = decode_canonical::<DecodedPropertyCapabilityV1>(
        &capability_wire(1, getter, Some((getter, 1))),
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 4,
        }
    );

    let error = decode_canonical::<DecodedPropertyCapabilityV1>(
        &[0xa1, 0x00, 0x03],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}

#[test]
fn decoded_capability_reports_getter_and_setter_resolution_failures() {
    let (getter, setter) = accessors();
    let mut read_only_wire = capability_wire(1, getter, None);
    read_only_wire[6] ^= 0xff;
    let decoded: DecodedPropertyCapabilityV1 =
        decode_canonical(&read_only_wire, DecodeLimits::default()).unwrap();
    let mut resolver = AccessorResolver { getter, setter };
    assert_eq!(
        decoded.resolve(&mut resolver),
        Err(PropertyCapabilityResolutionError::Getter(UnknownAccessor))
    );

    let mut read_write_wire = capability_wire(2, getter, Some((setter, 2)));
    read_write_wire[41] ^= 0xff;
    let decoded: DecodedPropertyCapabilityV1 =
        decode_canonical(&read_write_wire, DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded.resolve(&mut resolver),
        Err(PropertyCapabilityResolutionError::Setter(UnknownAccessor))
    );
}

fn accessors() -> (PersistentPropertyAccessorId, PersistentPropertyAccessorId) {
    let property =
        CborIdentityRecord::<PersistentPropertyId, _>::from_key(SourceDeclarationKey::property(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("value").unwrap(),
        ))
        .unwrap()
        .id();
    let owner = PropertyOwner::Property(property);
    (
        PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
            owner,
            AccessorRole::Getter,
        ))
        .unwrap(),
        PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
            owner,
            AccessorRole::Setter,
        ))
        .unwrap(),
    )
}

fn capability_wire(
    tag: u8,
    getter: PersistentPropertyAccessorId,
    setter: Option<(PersistentPropertyAccessorId, u8)>,
) -> Vec<u8> {
    let mut bytes = vec![
        if setter.is_some() { 0xa4 } else { 0xa2 },
        0x00,
        tag,
        0x01,
        0x58,
        0x20,
    ];
    bytes.extend_from_slice(getter.as_array());
    if let Some((setter, access)) = setter {
        bytes.extend_from_slice(&[0x02, 0x58, 0x20]);
        bytes.extend_from_slice(setter.as_array());
        bytes.extend_from_slice(&[0x03, access]);
    }
    bytes
}

struct AccessorResolver {
    getter: PersistentPropertyAccessorId,
    setter: PersistentPropertyAccessorId,
}

impl PersistentIdResolver<PersistentPropertyAccessorId> for AccessorResolver {
    type Error = UnknownAccessor;

    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<PersistentPropertyAccessorId>,
    ) -> Result<PersistentPropertyAccessorId, Self::Error> {
        if decoded.as_array() == self.getter.as_array() {
            Ok(self.getter)
        } else if decoded.as_array() == self.setter.as_array() {
            Ok(self.setter)
        } else {
            Err(UnknownAccessor)
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct UnknownAccessor;

impl fmt::Display for UnknownAccessor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("unknown property accessor")
    }
}

impl std::error::Error for UnknownAccessor {}
