use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    PackagePath, PersistentTypeAliasId, PersistentTypeId, SignatureTypeKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn variants_have_fixed_wire_and_resolve_typed_references() {
    let nominal = nominal("Target");
    let alias = alias("Alias");
    let mut authority = authority(&nominal, &alias);

    let signature = TypeAliasTargetV1::Signature(SignatureTypeKey::Nominal(nominal.id()));
    let expected = [
        b"\xa2\x00\x01\x01".as_slice(),
        encode(&SignatureTypeKey::Nominal(nominal.id()))
            .unwrap()
            .as_slice(),
    ]
    .concat();
    assert_eq!(encode(&signature).unwrap(), expected);
    assert_eq!(
        decode(&signature).resolve(&mut authority).unwrap(),
        signature
    );

    let target = TypeAliasTargetV1::Alias(alias.id());
    let expected = [
        b"\xa2\x00\x02\x01".as_slice(),
        encode(&alias.id()).unwrap().as_slice(),
    ]
    .concat();
    assert_eq!(encode(&target).unwrap(), expected);
    assert_eq!(decode(&target).resolve(&mut authority).unwrap(), target);
}

#[test]
fn decoder_rejects_unknown_tags_and_wrong_sum_shape() {
    let unknown = decode_canonical::<DecodedTypeAliasTargetV1>(
        &[0xa2, 0x00, 0x03, 0x01, 0x00],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let wrong_shape =
        decode_canonical::<DecodedTypeAliasTargetV1>(&[0xa1, 0x00, 0x01], DecodeLimits::default())
            .unwrap_err();
    assert_eq!(
        wrong_shape.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );
}

#[test]
fn resolution_rejects_missing_nominal_and_alias_authority() {
    let nominal = nominal("MissingTarget");
    let alias = alias("MissingAlias");
    let mut empty = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();

    assert!(matches!(
        decode(&TypeAliasTargetV1::Signature(SignatureTypeKey::Nominal(
            nominal.id()
        )))
        .resolve(&mut empty),
        Err(TypeAliasTargetResolutionError::Signature(_))
    ));
    assert!(matches!(
        decode(&TypeAliasTargetV1::Alias(alias.id())).resolve(&mut empty),
        Err(TypeAliasTargetResolutionError::Alias(_))
    ));
}

fn decode(target: &TypeAliasTargetV1) -> DecodedTypeAliasTargetV1 {
    decode_canonical(&encode(target).unwrap(), DecodeLimits::default()).unwrap()
}

fn authority(
    nominal: &CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    alias: &CborIdentityRecord<PersistentTypeAliasId, SourceDeclarationKey>,
) -> scoop_identity::ValidatedIdentityGraph {
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    pending
        .register_external_canonical_authority(nominal.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(alias.clone())
        .unwrap();
    pending.finish().unwrap()
}

fn nominal(name: &str) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(),
        identifier(name),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap()
}

fn alias(name: &str) -> CborIdentityRecord<PersistentTypeAliasId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::type_alias(site(), identifier(name)))
        .unwrap()
}

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
