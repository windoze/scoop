use std::num::NonZeroU64;

use scoop_wire::{WireEncode, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedCanonicalScoopAbiFunctionSignature, DecodedCanonicalScoopStorage,
    DecodedScoopAbiArgument, DecodedScoopAbiReturn, ScoopAbiResolutionError,
};
use crate::{
    CanonicalScoopAbiFunctionSignature, CanonicalScoopStorage, Effect, ExactCallableSignature,
    GcEffect, PersistentExactTypeId, PersistentIdMismatch, PersistentIdResolver, ScoopAbiArgument,
    ScoopAbiError, ScoopAbiReturn, ScoopAbiValueShape,
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

#[test]
fn scoop_storage_round_trips_and_resolves_exact_type() {
    let storage = scalar(first_type());
    let decoded =
        decode_canonical::<DecodedCanonicalScoopStorage>(&encode(&storage).unwrap()).unwrap();
    assert_eq!(decoded.resolve(&mut Resolver).unwrap(), storage);
}

#[test]
fn all_scoop_argument_and_return_kinds_round_trip_and_recheck_shape() {
    let arguments = [
        ScoopAbiArgument::elided_zst(zero(first_type())).unwrap(),
        ScoopAbiArgument::direct(scalar(first_type())).unwrap(),
        ScoopAbiArgument::indirect(aggregate(first_type())).unwrap(),
        ScoopAbiArgument::direct_parts(storage(first_type(), 16, ScoopAbiValueShape::Interface))
            .unwrap(),
    ];
    for argument in arguments {
        let decoded =
            decode_canonical::<DecodedScoopAbiArgument>(&encode(&argument).unwrap()).unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), argument);
    }

    let returns = [
        ScoopAbiReturn::unit_void(),
        ScoopAbiReturn::elided_zst(zero(second_type())).unwrap(),
        ScoopAbiReturn::direct(scalar(second_type())).unwrap(),
        ScoopAbiReturn::indirect(aggregate(second_type())).unwrap(),
        ScoopAbiReturn::direct_parts(storage(second_type(), 16, ScoopAbiValueShape::Interface))
            .unwrap(),
    ];
    for result in returns {
        let decoded = decode_canonical::<DecodedScoopAbiReturn>(&encode(&result).unwrap()).unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), result);
    }
}

#[test]
fn canonical_scoop_signature_round_trips_and_rechecks_exact_signature() {
    let signature = canonical_signature();
    let decoded =
        decode_canonical::<DecodedCanonicalScoopAbiFunctionSignature>(&encode(&signature).unwrap())
            .unwrap();
    assert_eq!(decoded.resolve(&mut Resolver).unwrap(), signature);
}

#[test]
fn scoop_argument_decoder_does_not_bypass_passing_constructor() {
    let raw = RawArgument {
        tag: 2,
        storage: aggregate(first_type()),
    };
    let decoded = decode_canonical::<DecodedScoopAbiArgument>(&encode(&raw).unwrap()).unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(ScoopAbiResolutionError::Shape(
            ScoopAbiError::PassingShapeMismatch
        ))
    );
}

#[test]
fn scoop_signature_decoder_does_not_bypass_signature_constructor() {
    let raw = RawSignature {
        signature: ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            vec![first_type()],
            second_type(),
        ),
        arguments: Vec::new(),
        result: ScoopAbiReturn::unit_void(),
        gc_effect: GcEffect::Managed,
    };
    let decoded =
        decode_canonical::<DecodedCanonicalScoopAbiFunctionSignature>(&encode(&raw).unwrap())
            .unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(ScoopAbiResolutionError::Shape(
            ScoopAbiError::ArgumentCountMismatch
        ))
    );
}

#[test]
fn scoop_storage_decoder_rejects_zero_alignment() {
    let mut bytes = encode(&scalar(first_type())).unwrap();
    let offset = bytes
        .windows(3)
        .position(|window| window == [0x03, 0x08, 0x04])
        .unwrap();
    bytes[offset + 1] = 0;
    let error = decode_canonical::<DecodedCanonicalScoopStorage>(&bytes).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::IntegerOutOfRange);
}

#[test]
fn scoop_abi_decoder_rejects_unknown_tags() {
    assert_unknown::<ScoopAbiValueShape>(b"\x04", 4);
    assert_unknown::<DecodedScoopAbiArgument>(&unknown_value_sum(5), 5);
    assert_unknown::<DecodedScoopAbiReturn>(&unknown_value_sum(6), 6);
}

fn assert_unknown<T: scoop_wire::WireDecode + std::fmt::Debug>(bytes: &[u8], tag: u64) {
    let error = decode_canonical::<T>(bytes).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag });
}

fn unknown_value_sum(tag: u8) -> Vec<u8> {
    let mut bytes = vec![0xa2, 0x00, tag, 0x01];
    bytes.extend(encode(&scalar(first_type())).unwrap());
    bytes
}

fn canonical_signature() -> CanonicalScoopAbiFunctionSignature {
    CanonicalScoopAbiFunctionSignature::new(
        ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            vec![first_type(), first_type(), first_type()],
            second_type(),
        ),
        vec![
            ScoopAbiArgument::elided_zst(zero(first_type())).unwrap(),
            ScoopAbiArgument::direct(scalar(first_type())).unwrap(),
            ScoopAbiArgument::indirect(aggregate(first_type())).unwrap(),
        ],
        ScoopAbiReturn::direct(scalar(second_type())).unwrap(),
        GcEffect::NoGc,
    )
    .unwrap()
}

fn zero(exact_type: PersistentExactTypeId) -> CanonicalScoopStorage {
    storage(exact_type, 0, ScoopAbiValueShape::Aggregate)
}

fn scalar(exact_type: PersistentExactTypeId) -> CanonicalScoopStorage {
    storage(exact_type, 8, ScoopAbiValueShape::Scalar)
}

fn aggregate(exact_type: PersistentExactTypeId) -> CanonicalScoopStorage {
    storage(exact_type, 8, ScoopAbiValueShape::Aggregate)
}

fn storage(
    exact_type: PersistentExactTypeId,
    byte_size: u64,
    shape: ScoopAbiValueShape,
) -> CanonicalScoopStorage {
    CanonicalScoopStorage::new(exact_type, byte_size, NonZeroU64::new(8).unwrap(), shape)
}

const fn first_type() -> PersistentExactTypeId {
    PersistentExactTypeId([1; 32])
}

const fn second_type() -> PersistentExactTypeId {
    PersistentExactTypeId([2; 32])
}

struct RawArgument {
    tag: u64,
    storage: CanonicalScoopStorage,
}

impl WireEncode for RawArgument {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(self.tag)?;
        encoder.field(1)?;
        self.storage.encode(encoder)
    }
}

struct RawSignature {
    signature: ExactCallableSignature,
    arguments: Vec<ScoopAbiArgument>,
    result: ScoopAbiReturn,
    gc_effect: GcEffect,
}

impl WireEncode for RawSignature {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.signature.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.arguments.len() as u64)?;
        for argument in &self.arguments {
            argument.encode(encoder)?;
        }
        encoder.field(3)?;
        self.result.encode(encoder)?;
        encoder.field(4)?;
        self.gc_effect.encode(encoder)
    }
}
