use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::{DecodedDuplicateSignatureKey, DecodedSignatureTypeKey};
use crate::{
    ConeIdentity, DuplicateSignatureKey, Effect, NonEmptyVec, OptionalSignatureType,
    PersistentGenericTypeId, PersistentIdMismatch, PersistentIdResolver, PersistentTypeId,
    SignatureTypeKey,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestResolveError {
    Type,
    GenericType,
}

struct TestResolver {
    nominal: PersistentTypeId,
    generic: PersistentGenericTypeId,
}

impl PersistentIdResolver<PersistentTypeId> for TestResolver {
    type Error = TestResolveError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        id.verify(self.nominal)
            .map_err(|_: PersistentIdMismatch<PersistentTypeId>| TestResolveError::Type)
    }
}

impl PersistentIdResolver<PersistentGenericTypeId> for TestResolver {
    type Error = TestResolveError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        id.verify(self.generic)
            .map_err(|_: PersistentIdMismatch<PersistentGenericTypeId>| {
                TestResolveError::GenericType
            })
    }
}

#[test]
fn signature_tree_round_trips_then_resolves_typed_references() {
    let nominal = PersistentTypeId(ConeIdentity::CORE.0);
    let generic = PersistentGenericTypeId(ConeIdentity::SINGLE_FILE.0);
    let binder = SignatureTypeKey::Binder { depth: 0, index: 1 };
    let key = DuplicateSignatureKey::Function {
        conditions: Vec::new(),
        type_parameter_count: 2,
        receiver: OptionalSignatureType::Present(Box::new(SignatureTypeKey::Nominal(nominal))),
        parameters: vec![
            SignatureTypeKey::NominalApplication {
                origin: generic,
                arguments: NonEmptyVec::from_first(binder.clone(), []),
            },
            SignatureTypeKey::Tuple(NonEmptyVec::from_first(
                SignatureTypeKey::RawPointer(Box::new(binder.clone())),
                [],
            )),
            SignatureTypeKey::Function {
                receiver: None,
                effect: Effect::Suspend,
                parameters: vec![binder.clone()],
                result: Box::new(binder),
            },
        ],
    };
    let bytes = encode(&key).unwrap();
    let decoded = decode_canonical::<DecodedDuplicateSignatureKey>(&bytes).unwrap();

    assert_eq!(
        decoded
            .resolve(&mut TestResolver { nominal, generic })
            .unwrap(),
        key
    );
}

#[test]
fn signature_decoder_rejects_unknown_tags_and_empty_required_sequences() {
    let unknown = decode_canonical::<DecodedSignatureTypeKey>(b"\xa1\x00\x09").unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 9 });

    let nominal = PersistentGenericTypeId(ConeIdentity::CORE.0);
    let mut empty_application = vec![0xa3, 0x00, 0x02, 0x01, 0x58, 0x20];
    empty_application.extend_from_slice(nominal.as_array());
    empty_application.extend_from_slice(&[0x02, 0x80]);
    let error = decode_canonical::<DecodedSignatureTypeKey>(&empty_application).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 0,
        }
    );
}

#[test]
fn signature_resolution_does_not_accept_a_same_width_wrong_identity() {
    let key = SignatureTypeKey::Nominal(PersistentTypeId(ConeIdentity::CORE.0));
    let bytes = encode(&key).unwrap();
    let decoded = decode_canonical::<DecodedSignatureTypeKey>(&bytes).unwrap();
    let error = decoded
        .resolve(&mut TestResolver {
            nominal: PersistentTypeId(ConeIdentity::SINGLE_FILE.0),
            generic: PersistentGenericTypeId(ConeIdentity::CORE.0),
        })
        .unwrap_err();
    assert_eq!(error, TestResolveError::Type);
}

#[test]
fn receiver_signature_preserves_binders_and_wire_shape() {
    let nominal = PersistentTypeId(ConeIdentity::CORE.0);
    let key = SignatureTypeKey::Function {
        effect: Effect::Suspend,
        parameters: vec![],
        result: Box::new(SignatureTypeKey::Nominal(nominal)),
        receiver: Some(Box::new(SignatureTypeKey::Binder { depth: 0, index: 0 })),
    };
    assert!(key.contains_binder());
    let bytes = encode(&key).unwrap();
    assert_eq!(bytes[0], 0xa5);
    let decoded = decode_canonical::<DecodedSignatureTypeKey>(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded
            .resolve(&mut TestResolver {
                nominal,
                generic: PersistentGenericTypeId(ConeIdentity::SINGLE_FILE.0),
            })
            .unwrap(),
        key
    );
}
