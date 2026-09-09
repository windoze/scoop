use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedSourceCAbiFunctionSignature, DecodedSourceExternFunctionAbi,
    DecodedSourceNativeLibraryBinding, DecodedSourceScoopAbiFunctionSignature,
};
use crate::{
    CallbackMode, CanonicalNativeLibraryName, CanonicalNativeNameError, GcEffect, NonEmptyVec,
    PersistentGenericTypeId, PersistentIdMismatch, PersistentIdResolver, PersistentTypeId,
    SignatureTypeKey, SourceCAbiFunctionSignature, SourceCAbiReturn, SourceCallingConvention,
    SourceExternFunctionAbi, SourceNativeLibraryBinding, SourceScoopAbiFunctionSignature,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

struct Resolver;

impl PersistentIdResolver<PersistentTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        id.verify(plain_type())
            .map_err(|_: PersistentIdMismatch<PersistentTypeId>| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentGenericTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        id.verify(generic_type())
            .map_err(|_: PersistentIdMismatch<PersistentGenericTypeId>| ResolutionError)
    }
}

#[test]
fn source_library_bindings_round_trip_and_validate() {
    let bindings = [
        SourceNativeLibraryBinding::DefaultNativeNamespace,
        SourceNativeLibraryBinding::LogicalLibrary(
            CanonicalNativeLibraryName::new("native-core").unwrap(),
        ),
    ];

    for binding in bindings {
        let decoded = decode_canonical::<DecodedSourceNativeLibraryBinding>(
            &encode(&binding).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.validate().unwrap(), binding);
    }

    let invalid = decode_canonical::<DecodedSourceNativeLibraryBinding>(
        b"\xa2\x00\x02\x01\x64a/bc",
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(
        invalid.validate(),
        Err(CanonicalNativeNameError::ForbiddenCharacter)
    );
}

#[test]
fn source_c_abi_signatures_round_trip_and_resolve_type_references() {
    let signatures = [
        SourceCAbiFunctionSignature::new(vec![nominal()], SourceCAbiReturn::Void),
        SourceCAbiFunctionSignature::new(
            vec![binder(), application()],
            SourceCAbiReturn::Value(nominal()),
        ),
    ];

    for signature in signatures {
        let decoded = decode_canonical::<DecodedSourceCAbiFunctionSignature>(
            &encode(&signature).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), signature);
    }
}

#[test]
fn source_scoop_abi_signatures_round_trip_and_resolve_type_references() {
    let signature = SourceScoopAbiFunctionSignature::new(vec![application(), binder()], nominal());
    let decoded = decode_canonical::<DecodedSourceScoopAbiFunctionSignature>(
        &encode(&signature).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(decoded.resolve(&mut Resolver).unwrap(), signature);
}

#[test]
fn both_source_extern_abis_round_trip_and_resolve() {
    let values = [
        SourceExternFunctionAbi::C(SourceCAbiFunctionSignature::new(
            vec![nominal()],
            SourceCAbiReturn::Void,
        )),
        SourceExternFunctionAbi::Scoop {
            signature: SourceScoopAbiFunctionSignature::new(vec![binder()], application()),
            gc_effect: GcEffect::NoGc,
        },
    ];

    for value in values {
        let decoded = decode_canonical::<DecodedSourceExternFunctionAbi>(
            &encode(&value).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), value);
    }
}

#[test]
fn source_abi_decoder_rejects_unknown_tags() {
    assert_unknown::<DecodedSourceNativeLibraryBinding>(b"\xa1\x00\x03", 3);
    assert_unknown::<super::DecodedSourceCAbiReturn>(b"\xa1\x00\x03", 3);
    assert_unknown::<DecodedSourceExternFunctionAbi>(b"\xa1\x00\x03", 3);
    assert_unknown::<GcEffect>(b"\x03", 3);
    assert_unknown::<SourceCallingConvention>(b"\x02", 2);
    assert_unknown::<CallbackMode>(b"\x03", 3);
}

fn assert_unknown<T: scoop_wire::WireDecode + std::fmt::Debug>(bytes: &[u8], tag: u64) {
    let error = decode_canonical::<T>(bytes, DecodeLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag });
}

fn nominal() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(plain_type())
}

fn application() -> SignatureTypeKey {
    SignatureTypeKey::NominalApplication {
        origin: generic_type(),
        arguments: NonEmptyVec::from_first(nominal(), []),
    }
}

const fn binder() -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index: 0 }
}

const fn plain_type() -> PersistentTypeId {
    PersistentTypeId([7; 32])
}

const fn generic_type() -> PersistentGenericTypeId {
    PersistentGenericTypeId([8; 32])
}
