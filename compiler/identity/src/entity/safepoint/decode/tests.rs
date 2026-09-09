use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::DecodedSafepointSiteKey;
use crate::{
    CborIdentityRecord, DecodedCborIdentityRecord, IdentityRecordResolutionError,
    PersistentCallableBodyId, PersistentExactTypeId, PersistentIdMismatch, PersistentIdResolver,
    PersistentSafepointSiteId, RuntimeTypeId, SafepointId, SafepointSiteKey, SafepointSiteRole,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

struct Resolver;

impl PersistentIdResolver<PersistentCallableBodyId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<PersistentCallableBodyId>,
    ) -> Result<PersistentCallableBodyId, Self::Error> {
        id.verify(body())
            .map_err(|_: PersistentIdMismatch<PersistentCallableBodyId>| ResolutionError)
    }
}

#[test]
fn safepoint_site_record_round_trips_and_resolves_its_body() {
    let key = SafepointSiteKey::new(body(), SafepointSiteRole::ManagedInvoke, 7);
    let record = CborIdentityRecord::<PersistentSafepointSiteId, _>::from_key(key).unwrap();
    let decoded = decode_canonical::<
        DecodedCborIdentityRecord<PersistentSafepointSiteId, DecodedSafepointSiteKey>,
    >(&encode(&record).unwrap(), DecodeLimits::default())
    .unwrap();
    let resolved = decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap();

    assert_eq!(resolved, record);
}

#[test]
fn safepoint_site_resolution_rejects_a_different_same_width_body() {
    let key = SafepointSiteKey::new(
        PersistentCallableBodyId([9; 32]),
        SafepointSiteRole::ManagedPoll,
        0,
    );
    let record = CborIdentityRecord::<PersistentSafepointSiteId, _>::from_key(key).unwrap();
    let decoded = decode_canonical::<
        DecodedCborIdentityRecord<PersistentSafepointSiteId, DecodedSafepointSiteKey>,
    >(&encode(&record).unwrap(), DecodeLimits::default())
    .unwrap();

    assert_eq!(
        decoded.resolve(|key| key.resolve(&mut Resolver)),
        Err(IdentityRecordResolutionError::Reference(ResolutionError))
    );
}

#[test]
fn safepoint_roles_round_trip_and_reject_unknown_tags() {
    let roles = [
        SafepointSiteRole::ManagedPoll,
        SafepointSiteRole::ManagedCall,
        SafepointSiteRole::ManagedInvoke,
        SafepointSiteRole::NativeSafeTransition,
        SafepointSiteRole::NativeBorrowedTransition,
    ];
    for role in roles {
        assert_eq!(
            decode_canonical::<SafepointSiteRole>(&encode(&role).unwrap(), DecodeLimits::default())
                .unwrap(),
            role
        );
    }

    let error =
        decode_canonical::<SafepointSiteRole>(b"\x06", DecodeLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 6 });
}

#[test]
fn derived_runtime_ids_round_trip_but_reject_zero() {
    let runtime_type = RuntimeTypeId::derive(PersistentExactTypeId([1; 32])).unwrap();
    assert_eq!(
        decode_canonical::<RuntimeTypeId>(&encode(&runtime_type).unwrap(), DecodeLimits::default())
            .unwrap(),
        runtime_type
    );

    let safepoint = SafepointId::derive(PersistentSafepointSiteId([2; 32])).unwrap();
    assert_eq!(
        decode_canonical::<SafepointId>(&encode(&safepoint).unwrap(), DecodeLimits::default())
            .unwrap(),
        safepoint
    );

    for kind in [
        decode_canonical::<RuntimeTypeId>(b"\x00", DecodeLimits::default()).unwrap_err(),
        decode_canonical::<SafepointId>(b"\x00", DecodeLimits::default()).unwrap_err(),
    ] {
        assert_eq!(kind.kind(), &WireErrorKind::IntegerOutOfRange);
    }
}

const fn body() -> PersistentCallableBodyId {
    PersistentCallableBodyId([7; 32])
}
