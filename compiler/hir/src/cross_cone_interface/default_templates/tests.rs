use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, PackagePath, PendingIdentityValidation,
    PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite, ValidatedIdentityGraph,
};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn key_has_fixed_wire_and_resolves_typed_owner() {
    let function = function("collect");
    let key = ExportDefaultTemplateKeyV1::new(CallableTemplateOrigin::Function(function.id()), 42);
    let bytes = encode(&key).unwrap();

    assert_eq!(
        hex(&bytes),
        "a201a2000101582012104f4f6e246d6533e24859a416718ca33e20fa88c78d1285c694aa56c3766402182a"
    );

    let decoded: DecodedExportDefaultTemplateKeyV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut authority(&function)).unwrap(), key);
}

#[test]
fn resolution_rejects_an_owner_without_identity_authority() {
    let key = ExportDefaultTemplateKeyV1::new(
        CallableTemplateOrigin::Function(function("missing").id()),
        0,
    );
    let decoded: DecodedExportDefaultTemplateKeyV1 =
        decode_canonical(&encode(&key).unwrap()).unwrap();
    let mut empty = PendingIdentityValidation::new().finish().unwrap();

    assert!(decoded.resolve(&mut empty).is_err());
}

#[test]
fn decoder_rejects_positions_larger_than_u32() {
    let function = function("wide");
    let mut bytes = vec![0xa2, 0x01];
    bytes.extend(encode(&CallableTemplateOrigin::Function(function.id())).unwrap());
    bytes.extend([0x02, 0x1b, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00]);

    let error = decode_canonical::<DecodedExportDefaultTemplateKeyV1>(&bytes).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::IntegerOutOfRange);
}

fn function(name: &str) -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn authority(
    function: &CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_authority(ConeIdentity::SINGLE_FILE)
        .unwrap();
    pending
        .register_external_canonical_authority(function.clone())
        .unwrap();
    pending.finish().unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
