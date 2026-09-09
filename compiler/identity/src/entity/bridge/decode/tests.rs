use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedGeneratedBridgeAtomKey, DecodedGeneratedBridgeAtomRoleKey,
    DecodedGeneratedBridgeSemanticTarget, DecodedGeneratedBridgeUnitKey,
};
use crate::{
    CallbackParameterIndex, CanonicalCAbiLayoutFingerprint, CanonicalCAbiSignatureFingerprint,
    CborIdentityRecord, ConeIdentity, DecodedCborIdentityRecord, GeneratedBridgeAtomId,
    GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey, GeneratedBridgeSemanticTarget,
    GeneratedBridgeUnitId, GeneratedBridgeUnitKey, NativeExternalContractFingerprint,
    PersistentIdMismatch, PersistentIdResolver,
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

id_resolver!(
    NativeExternalContractFingerprint,
    native_contract_fingerprint()
);
id_resolver!(CanonicalCAbiSignatureFingerprint, signature_fingerprint());
id_resolver!(CanonicalCAbiLayoutFingerprint, layout_fingerprint());
id_resolver!(GeneratedBridgeUnitId, bridge_unit());
id_resolver!(ConeIdentity, ConeIdentity::CORE);

#[test]
fn all_bridge_unit_keys_round_trip_and_resolve_typed_references() {
    let keys = [
        GeneratedBridgeUnitKey::OutboundFunction(native_contract_fingerprint()),
        GeneratedBridgeUnitKey::GlobalRead(native_contract_fingerprint()),
        GeneratedBridgeUnitKey::GlobalWrite(native_contract_fingerprint()),
        GeneratedBridgeUnitKey::GlobalAddress(native_contract_fingerprint()),
        GeneratedBridgeUnitKey::CallbackTrampoline {
            signature: signature_fingerprint(),
            context_index: CallbackParameterIndex::new(2),
        },
    ];

    for key in keys {
        let decoded = decode_canonical::<DecodedGeneratedBridgeUnitKey>(
            &encode(&key).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), key);
    }
}

#[test]
fn all_bridge_atom_roles_round_trip_and_resolve_typed_references() {
    let roles = [
        GeneratedBridgeAtomRoleKey::PrimaryEntry {
            unit: bridge_unit(),
        },
        GeneratedBridgeAtomRoleKey::SignatureDescriptor {
            unit: bridge_unit(),
            signature: signature_fingerprint(),
        },
        GeneratedBridgeAtomRoleKey::ContextDescriptor {
            unit: bridge_unit(),
            context_index: CallbackParameterIndex::new(3),
        },
        GeneratedBridgeAtomRoleKey::StaticAssertSupport {
            unit: bridge_unit(),
            layout: layout_fingerprint(),
        },
    ];

    for role in roles {
        let decoded = decode_canonical::<DecodedGeneratedBridgeAtomRoleKey>(
            &encode(&role).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), role);
    }
}

#[test]
fn bridge_unit_and_atom_records_resolve_before_verifying_identity() {
    let unit_key = GeneratedBridgeUnitKey::CallbackTrampoline {
        signature: signature_fingerprint(),
        context_index: CallbackParameterIndex::new(1),
    };
    let unit_record: CborIdentityRecord<GeneratedBridgeUnitId, _> =
        CborIdentityRecord::from_key(unit_key).unwrap();
    let decoded_unit = decode_canonical::<
        DecodedCborIdentityRecord<GeneratedBridgeUnitId, DecodedGeneratedBridgeUnitKey>,
    >(&encode(&unit_record).unwrap(), DecodeLimits::default())
    .unwrap();
    let resolved_unit = decoded_unit
        .resolve(|key| key.resolve(&mut Resolver))
        .unwrap();
    assert_eq!(resolved_unit.key(), &unit_key);

    let atom_key = GeneratedBridgeAtomKey::new(
        ConeIdentity::CORE,
        GeneratedBridgeAtomRoleKey::SignatureDescriptor {
            unit: bridge_unit(),
            signature: signature_fingerprint(),
        },
    );
    let atom_record: CborIdentityRecord<GeneratedBridgeAtomId, _> =
        CborIdentityRecord::from_key(atom_key).unwrap();
    let decoded_atom = decode_canonical::<
        DecodedCborIdentityRecord<GeneratedBridgeAtomId, DecodedGeneratedBridgeAtomKey>,
    >(&encode(&atom_record).unwrap(), DecodeLimits::default())
    .unwrap();
    let resolved_atom = decoded_atom
        .resolve(|key| key.resolve(&mut Resolver))
        .unwrap();
    assert_eq!(resolved_atom.key(), &atom_key);
}

#[test]
fn bridge_semantic_target_round_trips_through_a_typed_unit_reference() {
    let target = GeneratedBridgeSemanticTarget::new(bridge_unit());
    let decoded = decode_canonical::<DecodedGeneratedBridgeSemanticTarget>(
        &encode(&target).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();

    assert_eq!(decoded.resolve(&mut Resolver).unwrap(), target);
}

#[test]
fn bridge_resolution_rejects_a_different_same_width_reference() {
    let key = GeneratedBridgeUnitKey::GlobalRead(NativeExternalContractFingerprint([99; 32]));
    let decoded = decode_canonical::<DecodedGeneratedBridgeUnitKey>(
        &encode(&key).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();

    assert_eq!(decoded.resolve(&mut Resolver), Err(ResolutionError));
}

#[test]
fn bridge_decoder_rejects_unknown_and_incomplete_sums() {
    assert_unknown::<DecodedGeneratedBridgeUnitKey>(b"\xa1\x00\x06", 6);
    assert_unknown::<DecodedGeneratedBridgeAtomRoleKey>(b"\xa1\x00\x05", 5);

    let error =
        decode_canonical::<DecodedGeneratedBridgeUnitKey>(b"\xa1\x00\x01", DecodeLimits::default())
            .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1
        }
    );
}

fn assert_unknown<T: scoop_wire::WireDecode + std::fmt::Debug>(bytes: &[u8], tag: u64) {
    let error = decode_canonical::<T>(bytes, DecodeLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag });
}

const fn native_contract_fingerprint() -> NativeExternalContractFingerprint {
    NativeExternalContractFingerprint([1; 32])
}

const fn signature_fingerprint() -> CanonicalCAbiSignatureFingerprint {
    CanonicalCAbiSignatureFingerprint([2; 32])
}

const fn layout_fingerprint() -> CanonicalCAbiLayoutFingerprint {
    CanonicalCAbiLayoutFingerprint([3; 32])
}

const fn bridge_unit() -> GeneratedBridgeUnitId {
    GeneratedBridgeUnitId([4; 32])
}
