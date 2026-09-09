use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedDispatchSlotKey, DecodedDispatchTableKey, DecodedOptionalExactInterface,
    DispatchIdentityResolutionError,
};
use crate::{
    CborIdentityRecord, DecodedCborIdentityRecord, DecodedPersistentId, DispatchRole,
    DispatchSlotKey, DispatchTableKey, DispatchTableRole, PersistentDispatchSlotId,
    PersistentDispatchTableId, PersistentExactTypeId, PersistentFunctionId, PersistentIdMismatch,
    PersistentIdResolver, PersistentPropertyAccessorId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent from the test graph")
    }
}

impl std::error::Error for ResolutionError {}

struct Resolver;

impl PersistentIdResolver<PersistentFunctionId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentFunctionId>,
    ) -> Result<PersistentFunctionId, Self::Error> {
        id.verify(function())
            .map_err(|_: PersistentIdMismatch<PersistentFunctionId>| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentPropertyAccessorId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentPropertyAccessorId>,
    ) -> Result<PersistentPropertyAccessorId, Self::Error> {
        id.verify(accessor())
            .map_err(|_: PersistentIdMismatch<PersistentPropertyAccessorId>| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentExactTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        [exact_type(), interface()]
            .into_iter()
            .find(|expected| expected.as_array() == id.as_array())
            .ok_or(ResolutionError)
    }
}

#[test]
fn all_dispatch_slot_records_round_trip_and_resolve() {
    let keys = [
        DispatchSlotKey::virtual_method(function()),
        DispatchSlotKey::interface_method(function()),
        DispatchSlotKey::property_getter(accessor()),
        DispatchSlotKey::property_setter(accessor()),
    ];

    for key in keys {
        let record = CborIdentityRecord::<PersistentDispatchSlotId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentDispatchSlotId, DecodedDispatchSlotKey>,
        >(&encode(&record).unwrap(), DecodeLimits::default())
        .unwrap();
        assert_eq!(
            decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap(),
            record
        );
    }
}

#[test]
fn both_dispatch_table_records_round_trip_and_resolve() {
    let keys = [
        DispatchTableKey::vtable(exact_type()),
        DispatchTableKey::itable(exact_type(), interface()),
    ];

    for key in keys {
        let record = CborIdentityRecord::<PersistentDispatchTableId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentDispatchTableId, DecodedDispatchTableKey>,
        >(&encode(&record).unwrap(), DecodeLimits::default())
        .unwrap();
        assert_eq!(
            decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap(),
            record
        );
    }
}

#[test]
fn slot_resolution_rejects_owner_role_mismatches() {
    let mut function_bytes = encode(&DispatchSlotKey::virtual_method(function())).unwrap();
    *function_bytes.last_mut().unwrap() = 3;
    let function_key =
        decode_canonical::<DecodedDispatchSlotKey>(&function_bytes, DecodeLimits::default())
            .unwrap();
    assert_eq!(
        function_key.resolve(&mut Resolver),
        Err(DispatchIdentityResolutionError::InvalidFunctionRole(
            DispatchRole::PropertyGetter
        ))
    );

    let mut accessor_bytes = encode(&DispatchSlotKey::property_getter(accessor())).unwrap();
    *accessor_bytes.last_mut().unwrap() = 1;
    let accessor_key =
        decode_canonical::<DecodedDispatchSlotKey>(&accessor_bytes, DecodeLimits::default())
            .unwrap();
    assert_eq!(
        accessor_key.resolve(&mut Resolver),
        Err(DispatchIdentityResolutionError::InvalidAccessorRole(
            DispatchRole::VirtualMethod
        ))
    );
}

#[test]
fn table_resolution_rejects_role_interface_mismatches() {
    let mut vtable_bytes = encode(&DispatchTableKey::vtable(exact_type())).unwrap();
    assert_eq!(vtable_bytes[37], 1);
    vtable_bytes[37] = 2;
    let vtable_key =
        decode_canonical::<DecodedDispatchTableKey>(&vtable_bytes, DecodeLimits::default())
            .unwrap();
    assert_eq!(
        vtable_key.resolve(&mut Resolver),
        Err(DispatchIdentityResolutionError::ITableInterfaceAbsent)
    );

    let mut itable_bytes = encode(&DispatchTableKey::itable(exact_type(), interface())).unwrap();
    assert_eq!(itable_bytes[37], 2);
    itable_bytes[37] = 1;
    let itable_key =
        decode_canonical::<DecodedDispatchTableKey>(&itable_bytes, DecodeLimits::default())
            .unwrap();
    assert_eq!(
        itable_key.resolve(&mut Resolver),
        Err(DispatchIdentityResolutionError::VTableInterfacePresent)
    );
}

#[test]
fn dispatch_decoder_rejects_unknown_tags() {
    let slot_role = decode_canonical::<DispatchRole>(b"\x05", DecodeLimits::default()).unwrap_err();
    assert_eq!(slot_role.kind(), &WireErrorKind::UnknownTag { tag: 5 });

    let table_role =
        decode_canonical::<DispatchTableRole>(b"\x03", DecodeLimits::default()).unwrap_err();
    assert_eq!(table_role.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let interface =
        decode_canonical::<DecodedOptionalExactInterface>(b"\xa1\x00\x03", DecodeLimits::default())
            .unwrap_err();
    assert_eq!(interface.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}

const fn function() -> PersistentFunctionId {
    PersistentFunctionId([7; 32])
}

const fn accessor() -> PersistentPropertyAccessorId {
    PersistentPropertyAccessorId([8; 32])
}

const fn exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId([9; 32])
}

const fn interface() -> PersistentExactTypeId {
    PersistentExactTypeId([10; 32])
}
