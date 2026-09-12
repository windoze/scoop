use scoop_wire::{DecodeLimits, WireEncode, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedCanonicalCAbiFunctionSignature, DecodedCanonicalCAbiParameter,
    DecodedCanonicalCAbiReturn, DecodedCanonicalCAbiSignatureFingerprintRecord,
};
use crate::{
    CanonicalCAbiError, CanonicalCAbiFunctionSignature, CanonicalCAbiLayoutFingerprint,
    CanonicalCAbiParameter, CanonicalCAbiReturn, CanonicalCAbiSignatureFingerprintRecord,
    CanonicalCStorageType, IntegerBitWidth, PersistentExactTypeId, PersistentIdMismatch,
    PersistentIdResolver, Signedness, TargetCallingConvention,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

struct Resolver;

impl PersistentIdResolver<PersistentExactTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        id.verify(first_type())
            .or_else(|_| id.verify(second_type()))
            .map_err(|_: PersistentIdMismatch<PersistentExactTypeId>| ResolutionError)
    }
}

impl PersistentIdResolver<CanonicalCAbiLayoutFingerprint> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<CanonicalCAbiLayoutFingerprint>,
    ) -> Result<CanonicalCAbiLayoutFingerprint, Self::Error> {
        id.verify(referenced_layout())
            .map_err(|_: PersistentIdMismatch<CanonicalCAbiLayoutFingerprint>| ResolutionError)
    }
}

#[test]
fn c_abi_signature_round_trips_and_resolves() {
    let signature = signature();
    let decoded = decode_canonical::<DecodedCanonicalCAbiFunctionSignature>(
        &encode(&signature).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();

    assert_eq!(decoded.resolve(&mut Resolver).unwrap(), signature);
}

#[test]
fn c_abi_parameter_and_return_recheck_storage_exact_type() {
    let mismatched_storage = CanonicalCStorageType::Boolean {
        exact_type: second_type(),
    };
    let parameter = decode_canonical::<DecodedCanonicalCAbiParameter>(
        &encode(&RawParameter {
            source_exact_type: first_type(),
            storage: mismatched_storage,
        })
        .unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(
        parameter.resolve(&mut Resolver),
        Err(super::super::CanonicalCAbiResolutionError::Shape(
            CanonicalCAbiError::StorageExactTypeMismatch
        ))
    );

    let result = decode_canonical::<DecodedCanonicalCAbiReturn>(
        &encode(&RawReturn {
            source_exact_type: first_type(),
            storage: mismatched_storage,
        })
        .unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(
        result.resolve(&mut Resolver),
        Err(super::super::CanonicalCAbiResolutionError::Shape(
            CanonicalCAbiError::StorageExactTypeMismatch
        ))
    );
}

#[test]
fn c_abi_signature_fingerprint_record_round_trips_and_verifies_hash() {
    let record = CanonicalCAbiSignatureFingerprintRecord::new(signature()).unwrap();
    let decoded = decode_canonical::<DecodedCanonicalCAbiSignatureFingerprintRecord>(
        &encode(&record).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(
        decoded.candidate_fingerprint().unwrap(),
        record.fingerprint()
    );
    assert_eq!(decoded.resolve(&mut Resolver).unwrap(), record);

    let mut bytes = encode(&record).unwrap();
    assert_eq!(&bytes[..4], &[0xa2, 0x01, 0x58, 0x20]);
    bytes[4] ^= 1;
    let decoded = decode_canonical::<DecodedCanonicalCAbiSignatureFingerprintRecord>(
        &bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        decoded.resolve(&mut Resolver),
        Err(super::super::CanonicalCAbiResolutionError::SignatureFingerprint(_))
    ));
}

#[test]
fn c_abi_signature_decoder_rejects_unknown_calling_convention() {
    let error =
        decode_canonical::<TargetCallingConvention>(b"\x02", DecodeLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 2 });
}

fn signature() -> CanonicalCAbiFunctionSignature {
    CanonicalCAbiFunctionSignature::cdecl(
        vec![
            CanonicalCAbiParameter::new(
                first_type(),
                CanonicalCStorageType::Boolean {
                    exact_type: first_type(),
                },
            )
            .unwrap(),
            CanonicalCAbiParameter::new(
                second_type(),
                CanonicalCStorageType::Integer {
                    exact_type: second_type(),
                    signedness: Signedness::Signed,
                    bit_width: IntegerBitWidth::Bits64,
                },
            )
            .unwrap(),
        ],
        CanonicalCAbiReturn::value(
            first_type(),
            CanonicalCStorageType::Boolean {
                exact_type: first_type(),
            },
        )
        .unwrap(),
    )
}

const fn first_type() -> PersistentExactTypeId {
    PersistentExactTypeId([1; 32])
}

const fn second_type() -> PersistentExactTypeId {
    PersistentExactTypeId([2; 32])
}

const fn referenced_layout() -> CanonicalCAbiLayoutFingerprint {
    CanonicalCAbiLayoutFingerprint([4; 32])
}

struct RawParameter {
    source_exact_type: PersistentExactTypeId,
    storage: CanonicalCStorageType,
}

impl WireEncode for RawParameter {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.source_exact_type.encode(encoder)?;
        encoder.field(2)?;
        self.storage.encode(encoder)
    }
}

struct RawReturn {
    source_exact_type: PersistentExactTypeId,
    storage: CanonicalCStorageType,
}

impl WireEncode for RawReturn {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(2)?;
        encoder.field(1)?;
        self.source_exact_type.encode(encoder)?;
        encoder.field(2)?;
        self.storage.encode(encoder)
    }
}
