use scoop_wire::{Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedGeneratedBridgeAtomKey, DecodedGeneratedBridgeAtomRoleKey,
    DecodedGeneratedBridgeSemanticTarget, DecodedGeneratedBridgeUnitKey,
};
use crate::{
    CallbackParameterIndex, CanonicalCAbiLayoutFingerprint, CanonicalCAbiSignatureFingerprint,
    CborIdentityRecord, ConeIdentity, DecodedCborIdentityRecord, GeneratedBridgeAtomId,
    GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey, GeneratedBridgeSemanticTarget,
    GeneratedBridgeUnitId, GeneratedBridgeUnitKey, GeneratedBridgeUnitResolutionError,
    GeneratedCallableKey, NativeExternalContractFingerprint, PersistentGeneratedCallableId,
    PersistentIdMismatch, PersistentIdResolver, PersistentKeyResolver,
    StaticNoGcCallbackStorageBridgeId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

struct Resolver;

struct WrongStaticRoleResolver;

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

impl PersistentKeyResolver<PersistentGeneratedCallableId, GeneratedCallableKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: crate::DecodedPersistentId<PersistentGeneratedCallableId>,
    ) -> Result<std::sync::Arc<GeneratedCallableKey>, Self::Error> {
        let key = static_storage_bridge_key();
        id.verify(PersistentGeneratedCallableId::from_key(&key).unwrap())
            .map_err(|_: PersistentIdMismatch<PersistentGeneratedCallableId>| ResolutionError)?;
        Ok(std::sync::Arc::new(key))
    }
}

macro_rules! forward_id_resolver {
    ($id:ty) => {
        impl PersistentIdResolver<$id> for WrongStaticRoleResolver {
            type Error = ResolutionError;

            fn resolve(&mut self, id: crate::DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                <Resolver as PersistentIdResolver<$id>>::resolve(&mut Resolver, id)
            }
        }
    };
}

forward_id_resolver!(NativeExternalContractFingerprint);
forward_id_resolver!(CanonicalCAbiSignatureFingerprint);

impl PersistentKeyResolver<PersistentGeneratedCallableId, GeneratedCallableKey>
    for WrongStaticRoleResolver
{
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: crate::DecodedPersistentId<PersistentGeneratedCallableId>,
    ) -> Result<std::sync::Arc<GeneratedCallableKey>, Self::Error> {
        let key = wrong_static_storage_bridge_key();
        id.verify(PersistentGeneratedCallableId::from_key(&key).unwrap())
            .map_err(|_: PersistentIdMismatch<PersistentGeneratedCallableId>| ResolutionError)?;
        Ok(std::sync::Arc::new(key))
    }
}

struct RawStaticCallbackTrampoline {
    storage_bridge: PersistentGeneratedCallableId,
    signature: CanonicalCAbiSignatureFingerprint,
}

impl WireEncode for RawStaticCallbackTrampoline {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(6)?;
        encoder.field(1)?;
        self.storage_bridge.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)
    }
}

#[test]
fn all_bridge_unit_keys_round_trip_and_resolve_typed_references() {
    let keys = [
        GeneratedBridgeUnitKey::OutboundFunction(
            native_contract_fingerprint(),
            crate::CResultAdaptation::Direct,
        ),
        GeneratedBridgeUnitKey::OutboundFunction(
            native_contract_fingerprint(),
            crate::CResultAdaptation::CaptureErrno,
        ),
        GeneratedBridgeUnitKey::GlobalRead(native_contract_fingerprint()),
        GeneratedBridgeUnitKey::GlobalWrite(native_contract_fingerprint()),
        GeneratedBridgeUnitKey::GlobalAddress(native_contract_fingerprint()),
        GeneratedBridgeUnitKey::CallbackTrampoline {
            signature: signature_fingerprint(),
            context_index: CallbackParameterIndex::new(2),
        },
        GeneratedBridgeUnitKey::StaticCallbackTrampoline {
            storage_bridge: static_storage_bridge(),
            signature: signature_fingerprint(),
        },
    ];

    for key in keys {
        let decoded =
            decode_canonical::<DecodedGeneratedBridgeUnitKey>(&encode(&key).unwrap()).unwrap();
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
        let decoded =
            decode_canonical::<DecodedGeneratedBridgeAtomRoleKey>(&encode(&role).unwrap()).unwrap();
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
    >(&encode(&unit_record).unwrap())
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
    >(&encode(&atom_record).unwrap())
    .unwrap();
    let resolved_atom = decoded_atom
        .resolve(|key| key.resolve(&mut Resolver))
        .unwrap();
    assert_eq!(resolved_atom.key(), &atom_key);
}

#[test]
fn bridge_semantic_target_round_trips_through_a_typed_unit_reference() {
    let target = GeneratedBridgeSemanticTarget::new(bridge_unit());
    let decoded =
        decode_canonical::<DecodedGeneratedBridgeSemanticTarget>(&encode(&target).unwrap())
            .unwrap();

    assert_eq!(decoded.resolve(&mut Resolver).unwrap(), target);
}

#[test]
fn bridge_resolution_rejects_a_different_same_width_reference() {
    let key = GeneratedBridgeUnitKey::GlobalRead(NativeExternalContractFingerprint([99; 32]));
    let decoded =
        decode_canonical::<DecodedGeneratedBridgeUnitKey>(&encode(&key).unwrap()).unwrap();

    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(GeneratedBridgeUnitResolutionError::Reference(
            ResolutionError
        ))
    );
}

#[test]
fn static_trampoline_rejects_a_non_storage_generated_callable() {
    let wrong_key = wrong_static_storage_bridge_key();
    let key = RawStaticCallbackTrampoline {
        storage_bridge: PersistentGeneratedCallableId::from_key(&wrong_key).unwrap(),
        signature: signature_fingerprint(),
    };
    let decoded =
        decode_canonical::<DecodedGeneratedBridgeUnitKey>(&encode(&key).unwrap()).unwrap();

    assert_eq!(
        decoded.resolve(&mut WrongStaticRoleResolver),
        Err(GeneratedBridgeUnitResolutionError::StaticStorageBridge(
            crate::StaticNoGcCallbackStorageBridgeIdentityError::WrongGeneratedRole
        ))
    );
}

#[test]
fn bridge_decoder_rejects_unknown_and_incomplete_sums() {
    assert_unknown::<DecodedGeneratedBridgeUnitKey>(b"\xa1\x00\x07", 7);
    assert_unknown::<DecodedGeneratedBridgeAtomRoleKey>(b"\xa1\x00\x05", 5);

    let error = decode_canonical::<DecodedGeneratedBridgeUnitKey>(b"\xa1\x00\x01").unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 3,
            actual: 1
        }
    );
    let mut old = encode(&GeneratedBridgeUnitKey::OutboundFunction(
        native_contract_fingerprint(),
        crate::CResultAdaptation::Direct,
    ))
    .unwrap();
    old[0] = 0xa2;
    old.truncate(old.len() - 2);
    assert!(decode_canonical::<DecodedGeneratedBridgeUnitKey>(&old).is_err());
}

fn assert_unknown<T: scoop_wire::WireDecode + std::fmt::Debug>(bytes: &[u8], tag: u64) {
    let error = decode_canonical::<T>(bytes).unwrap_err();
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

fn static_storage_bridge() -> StaticNoGcCallbackStorageBridgeId {
    StaticNoGcCallbackStorageBridgeId::from_key(&static_storage_bridge_key()).unwrap()
}

fn static_storage_bridge_key() -> GeneratedCallableKey {
    use crate::{
        CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner, Effect,
        ExactCallableSignature, PersistentExactTypeId, PersistentFunctionId,
    };

    GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
        source: CallableMaterialization::new(
            CallableTemplateOwner::Function(PersistentFunctionId([5; 32])),
            CallableMaterializationContext::NoSubstitution,
        ),
        signature: ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            Vec::new(),
            PersistentExactTypeId([6; 32]),
        ),
    }
}

fn wrong_static_storage_bridge_key() -> GeneratedCallableKey {
    GeneratedCallableKey::CoroutineStart {
        result: crate::PersistentExactTypeId([9; 32]),
    }
}
