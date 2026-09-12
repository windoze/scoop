use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedNativeExternAbi, DecodedNativeExternalContract, DecodedNativeExternalContractRecord,
    NativeExternalContractResolutionError,
};
use crate::{
    CanonicalCAbiFunctionSignature, CanonicalCAbiLayoutFingerprint, CanonicalCAbiReturn,
    CanonicalCStorageType, CanonicalScoopAbiFunctionSignature, Effect, ExactCallableSignature,
    GcEffect, NativeExternAbi, NativeExternalContract, NativeExternalContractRecord,
    NativeExternalSymbolKey, NativeLibraryBinding, NativeLinkRequirementId,
    NativeLinkValidationError, PersistentExactTypeId, PersistentIdMismatch, PersistentIdResolver,
    PersistentSourceNativeExternalContractId, ScoopAbiReturn, SourceNativeSymbol,
    TargetCallingConvention,
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

id_resolver!(PersistentSourceNativeExternalContractId, source_contract());
id_resolver!(NativeLinkRequirementId, link_requirement());
id_resolver!(PersistentExactTypeId, exact_type());
id_resolver!(CanonicalCAbiLayoutFingerprint, layout_fingerprint());

#[test]
fn both_native_function_abis_round_trip_and_resolve() {
    let values = [
        NativeExternAbi::C(c_signature()),
        NativeExternAbi::Scoop(scoop_signature()),
    ];

    for value in values {
        let decoded = decode_canonical::<DecodedNativeExternAbi>(
            &encode(&value).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), value);
    }
}

#[test]
fn all_native_contract_kinds_round_trip_and_resolve() {
    let mut values = data_contracts();
    values.insert(0, c_function_contract());
    values.insert(1, scoop_function_contract());

    for value in values {
        let decoded = decode_canonical::<DecodedNativeExternalContract>(
            &encode(&value).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), value);
    }
}

#[test]
fn native_contract_record_round_trips_and_verifies_derived_ids() {
    let record = record();
    let decoded = decode_canonical::<DecodedNativeExternalContractRecord>(
        &encode(&record).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();

    assert_eq!(
        decoded.into_candidate_fingerprint().unwrap(),
        record.fingerprint()
    );
    let decoded = decode_canonical::<DecodedNativeExternalContractRecord>(
        &encode(&record).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(decoded.resolve(&mut Resolver).unwrap(), record);
}

#[test]
fn native_contract_record_rejects_wrong_symbol_id_and_fingerprint() {
    let record = record();
    let mut symbol_bytes = encode(&record).unwrap();
    mutate_id(&mut symbol_bytes, record.symbol_id().as_array());
    let decoded = decode_canonical::<DecodedNativeExternalContractRecord>(
        &symbol_bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        decoded.resolve(&mut Resolver),
        Err(NativeExternalContractResolutionError::SymbolId(_))
    ));

    let mut fingerprint_bytes = encode(&record).unwrap();
    mutate_id(&mut fingerprint_bytes, record.fingerprint().as_array());
    let decoded = decode_canonical::<DecodedNativeExternalContractRecord>(
        &fingerprint_bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        decoded.resolve(&mut Resolver),
        Err(NativeExternalContractResolutionError::Fingerprint(_))
    ));
}

#[test]
fn native_contract_record_revalidates_symbol_key() {
    let record = record();
    let mut bytes = encode(&record).unwrap();
    replace_once(&mut bytes, b"_entry", b"xentry");
    let decoded =
        decode_canonical::<DecodedNativeExternalContractRecord>(&bytes, DecodeLimits::default())
            .unwrap();

    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(NativeExternalContractResolutionError::SymbolKey(
            NativeLinkValidationError::MissingMachOExternalPrefix
        ))
    );
}

#[test]
fn native_contract_record_resolves_source_as_a_typed_reference() {
    let record = record();
    let mut bytes = encode(&record).unwrap();
    mutate_id(&mut bytes, record.source().as_array());
    let decoded =
        decode_canonical::<DecodedNativeExternalContractRecord>(&bytes, DecodeLimits::default())
            .unwrap();

    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(NativeExternalContractResolutionError::Reference(
            ResolutionError
        ))
    );
}

#[test]
fn native_contract_decoder_rejects_unknown_tags() {
    let mut abi = encode(&NativeExternAbi::C(c_signature())).unwrap();
    assert_eq!(&abi[..3], &[0xa2, 0x00, 0x01]);
    abi[2] = 3;
    let error =
        decode_canonical::<DecodedNativeExternAbi>(&abi, DecodeLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let error =
        decode_canonical::<DecodedNativeExternalContract>(b"\xa1\x00\x06", DecodeLimits::default())
            .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 6 });
}

fn c_function_contract() -> NativeExternalContract {
    NativeExternalContract::c_function(NativeLibraryBinding::DefaultNativeNamespace, c_signature())
}

fn scoop_function_contract() -> NativeExternalContract {
    NativeExternalContract::scoop_function(
        NativeLibraryBinding::Requirement(link_requirement()),
        scoop_signature(),
        TargetCallingConvention::Cdecl,
    )
}

fn data_contracts() -> Vec<NativeExternalContract> {
    vec![
        NativeExternalContract::read_only_data(
            NativeLibraryBinding::DefaultNativeNamespace,
            storage(),
        ),
        NativeExternalContract::mutable_data(
            NativeLibraryBinding::Requirement(link_requirement()),
            storage(),
        ),
        NativeExternalContract::read_only_tls(
            NativeLibraryBinding::DefaultNativeNamespace,
            storage(),
        ),
        NativeExternalContract::mutable_tls(
            NativeLibraryBinding::Requirement(link_requirement()),
            storage(),
        ),
    ]
}

fn c_signature() -> CanonicalCAbiFunctionSignature {
    CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void)
}

fn scoop_signature() -> CanonicalScoopAbiFunctionSignature {
    CanonicalScoopAbiFunctionSignature::new(
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact_type()),
        Vec::new(),
        ScoopAbiReturn::unit_void(),
        GcEffect::Managed,
    )
    .unwrap()
}

fn storage() -> CanonicalCStorageType {
    CanonicalCStorageType::CodePointer {
        exact_type: exact_type(),
        storage: crate::CPointerStorage::Direct,
    }
}

fn record() -> NativeExternalContractRecord {
    NativeExternalContractRecord::new(
        source_contract(),
        symbol_key(),
        NativeExternalContract::read_only_data(
            NativeLibraryBinding::Requirement(link_requirement()),
            CanonicalCStorageType::Struct {
                exact_type: exact_type(),
                layout: layout_fingerprint(),
            },
        ),
    )
    .unwrap()
}

fn symbol_key() -> NativeExternalSymbolKey {
    NativeExternalSymbolKey::darwin_macho_external(&SourceNativeSymbol::new("entry").unwrap())
        .unwrap()
}

fn mutate_id(bytes: &mut [u8], id: &[u8; 32]) {
    let offset = bytes
        .windows(id.len())
        .position(|window| window == id)
        .unwrap();
    bytes[offset] ^= 1;
}

fn replace_once(bytes: &mut [u8], source: &[u8], replacement: &[u8]) {
    assert_eq!(source.len(), replacement.len());
    let offset = bytes
        .windows(source.len())
        .position(|window| window == source)
        .unwrap();
    bytes[offset..offset + source.len()].copy_from_slice(replacement);
}

const fn source_contract() -> PersistentSourceNativeExternalContractId {
    PersistentSourceNativeExternalContractId([9; 32])
}

const fn link_requirement() -> NativeLinkRequirementId {
    NativeLinkRequirementId([8; 32])
}

const fn exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId([1; 32])
}

const fn layout_fingerprint() -> CanonicalCAbiLayoutFingerprint {
    CanonicalCAbiLayoutFingerprint([3; 32])
}
