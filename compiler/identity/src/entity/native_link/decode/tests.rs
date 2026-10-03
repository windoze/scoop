use scoop_wire::{WireEncode, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedNativeExternalSymbolKey, DecodedNativeLibraryBinding, DecodedNativeLibraryGrouping,
    DecodedNativeLinkRequirementKey, DecodedNativeLinkSymbol, NativeLinkValidationError,
};
use crate::{
    CanonicalNativeGroupName, CanonicalNativeLibraryName, CanonicalNativeNameError, CapabilityId,
    CborIdentityRecord, DecodedCborIdentityRecord, NativeExternalSymbolKey, NativeLibraryBinding,
    NativeLibraryGrouping, NativeLibraryKind, NativeLinkRequirementId, NativeLinkRequirementKey,
    NativeLinkSymbolError, PersistentIdMismatch, PersistentIdResolver,
    PersistentNativeExternalSymbolId, SourceNativeSymbol, SourceNativeSymbolError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

struct Resolver;

impl PersistentIdResolver<NativeLinkRequirementId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<NativeLinkRequirementId>,
    ) -> Result<NativeLinkRequirementId, Self::Error> {
        id.verify(requirement_id())
            .map_err(|_: PersistentIdMismatch<NativeLinkRequirementId>| ResolutionError)
    }
}

#[test]
fn native_external_symbol_key_round_trips_and_validates_normalization() {
    let key = symbol_key();
    let decoded =
        decode_canonical::<DecodedNativeExternalSymbolKey>(&encode(&key).unwrap()).unwrap();
    let allocation = decoded.native_link_symbol.0.as_ptr();
    let decoded = decoded.validate().unwrap();
    assert_eq!(decoded.native_link_symbol().as_bytes().as_ptr(), allocation);
    assert_eq!(decoded, key);

    let record: CborIdentityRecord<PersistentNativeExternalSymbolId, _> =
        CborIdentityRecord::from_key(key.clone()).unwrap();
    let decoded = decode_canonical::<
        DecodedCborIdentityRecord<PersistentNativeExternalSymbolId, DecodedNativeExternalSymbolKey>,
    >(&encode(&record).unwrap())
    .unwrap();
    let resolved = decoded
        .resolve(DecodedNativeExternalSymbolKey::validate)
        .unwrap();
    assert_eq!(resolved.key(), &key);
}

#[test]
fn native_link_symbol_rejects_noncanonical_logical_inputs() {
    let missing_prefix = decode_canonical::<DecodedNativeLinkSymbol>(b"\x43foo").unwrap();
    assert_eq!(
        missing_prefix.validate_darwin(),
        Err(NativeLinkValidationError::MissingMachOExternalPrefix)
    );

    let invalid_utf8 = decode_canonical::<DecodedNativeLinkSymbol>(b"\x42_\xff").unwrap();
    assert_eq!(
        invalid_utf8.validate_darwin(),
        Err(NativeLinkValidationError::SourceSymbol(
            SourceNativeSymbolError::InvalidUtf8
        ))
    );

    let llvm_escape = decode_canonical::<DecodedNativeLinkSymbol>(b"\x45_\x01bad").unwrap();
    assert_eq!(
        llvm_escape.validate_darwin(),
        Err(NativeLinkValidationError::LinkSymbol(
            NativeLinkSymbolError::LlvmEscapePrefix
        ))
    );
}

#[test]
fn all_native_link_requirement_shapes_round_trip_and_validate() {
    let independent = NativeLibraryGrouping::Independent;
    let ordered = NativeLibraryGrouping::OrderedGroup {
        name: CanonicalNativeGroupName::new("runtime-group").unwrap(),
        position: 2,
    };
    let values = [
        NativeLinkRequirementKey::target_default(library()),
        NativeLinkRequirementKey::for_darwin(
            library(),
            NativeLibraryKind::Dynamic,
            ordered.clone(),
        ),
        NativeLinkRequirementKey::for_darwin(
            library(),
            NativeLibraryKind::StaticArchive,
            independent,
        ),
        NativeLinkRequirementKey::for_darwin(library(), NativeLibraryKind::Framework, ordered),
    ];

    for value in values {
        let decoded =
            decode_canonical::<DecodedNativeLinkRequirementKey>(&encode(&value).unwrap()).unwrap();
        assert_eq!(decoded.validate().unwrap(), value);
    }
}

#[test]
fn native_link_requirement_record_validates_before_verifying_identity() {
    let key = NativeLinkRequirementKey::for_darwin(
        library(),
        NativeLibraryKind::Dynamic,
        NativeLibraryGrouping::Independent,
    );
    let record: CborIdentityRecord<NativeLinkRequirementId, _> =
        CborIdentityRecord::from_key(key.clone()).unwrap();
    let decoded = decode_canonical::<
        DecodedCborIdentityRecord<NativeLinkRequirementId, DecodedNativeLinkRequirementKey>,
    >(&encode(&record).unwrap())
    .unwrap();
    let resolved = decoded
        .resolve(DecodedNativeLinkRequirementKey::validate)
        .unwrap();
    assert_eq!(resolved.key(), &key);
}

#[test]
fn native_library_bindings_round_trip_and_resolve_requirement_ids() {
    let values = [
        NativeLibraryBinding::DefaultNativeNamespace,
        NativeLibraryBinding::Requirement(requirement_id()),
    ];
    for value in values {
        let decoded =
            decode_canonical::<DecodedNativeLibraryBinding>(&encode(&value).unwrap()).unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), value);
    }
}

#[test]
fn native_link_keys_reject_wrong_profile_and_invalid_names() {
    let raw = RawExternalSymbolKey {
        target: CapabilityId::new("org.scoop-lang.target-profile", "darwin-aarch64", 2).unwrap(),
        symbol: b"_entry".to_vec(),
    };
    let decoded =
        decode_canonical::<DecodedNativeExternalSymbolKey>(&encode(&raw).unwrap()).unwrap();
    assert!(matches!(
        decoded.validate(),
        Err(NativeLinkValidationError::TargetProfile(_))
    ));

    let key = NativeLinkRequirementKey::for_darwin(
        CanonicalNativeLibraryName::new("sample").unwrap(),
        NativeLibraryKind::Dynamic,
        NativeLibraryGrouping::OrderedGroup {
            name: CanonicalNativeGroupName::new("group").unwrap(),
            position: 0,
        },
    );
    let mut library_bytes = encode(&key).unwrap();
    replace_once(&mut library_bytes, b"sample", b"bad/li");
    let decoded = decode_canonical::<DecodedNativeLinkRequirementKey>(&library_bytes).unwrap();
    assert_eq!(
        decoded.validate(),
        Err(NativeLinkValidationError::Library(
            CanonicalNativeNameError::ForbiddenCharacter
        ))
    );

    let mut group_bytes = encode(&key).unwrap();
    replace_once(&mut group_bytes, b"group", b"bad/g");
    let decoded = decode_canonical::<DecodedNativeLinkRequirementKey>(&group_bytes).unwrap();
    assert_eq!(
        decoded.validate(),
        Err(NativeLinkValidationError::Group(
            CanonicalNativeNameError::ForbiddenCharacter
        ))
    );
}

#[test]
fn native_link_decoder_rejects_unknown_kinds_and_grouping_tags() {
    let error = decode_canonical::<NativeLibraryKind>(b"\x05").unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 5 });

    let error = decode_canonical::<DecodedNativeLibraryGrouping>(b"\xa1\x00\x03").unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}

fn replace_once(bytes: &mut [u8], source: &[u8], replacement: &[u8]) {
    assert_eq!(source.len(), replacement.len());
    let offset = bytes
        .windows(source.len())
        .position(|window| window == source)
        .unwrap();
    bytes[offset..offset + source.len()].copy_from_slice(replacement);
}

fn symbol_key() -> NativeExternalSymbolKey {
    NativeExternalSymbolKey::darwin_macho_external(&SourceNativeSymbol::new("entry").unwrap())
        .unwrap()
}

fn library() -> CanonicalNativeLibraryName {
    CanonicalNativeLibraryName::new("sample").unwrap()
}

const fn requirement_id() -> NativeLinkRequirementId {
    NativeLinkRequirementId([7; 32])
}

struct RawExternalSymbolKey {
    target: CapabilityId,
    symbol: Vec<u8>,
}

impl WireEncode for RawExternalSymbolKey {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        encoder.bytes(&self.symbol)
    }
}
