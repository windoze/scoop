use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{
    CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity, DeclarationScope,
    DecodedCborIdentityRecord, DecodedExactTypeKey, DecodedSourceDeclarationKey,
    DefinitionOwnerChain, ExactTypeKey, PackagePath, PersistentExactTypeId, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

fn source_type_record() -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    let cone = ConeCoordinate::reserved_core().identity().unwrap();
    let site = SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let name = CanonicalIdentifier::new("Widget").unwrap();
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site,
        name,
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap()
}

fn decoded_source_type() -> DecodedCborIdentityRecord<PersistentTypeId, DecodedSourceDeclarationKey>
{
    decode_canonical(
        &encode(&source_type_record()).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
}

fn exact_type_record() -> CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
    CborIdentityRecord::from_key(ExactTypeKey::Nominal(source_type_record().id())).unwrap()
}

fn decoded_exact_type() -> DecodedCborIdentityRecord<PersistentExactTypeId, DecodedExactTypeKey> {
    decode_canonical(
        &encode(&exact_type_record()).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
}

#[test]
fn commits_a_complete_identity_transaction() {
    let decoded = decoded_source_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending.register(IdentityLayer::Hir, &decoded).unwrap();
    pending.resolve(&decoded).unwrap();

    let graph = pending.finish().unwrap();
    let records = graph
        .records::<PersistentTypeId, SourceDeclarationKey>(IdentityLayer::Hir)
        .unwrap();
    assert_eq!(records, vec![source_type_record()]);
}

#[test]
fn duplicate_registration_poisons_the_transaction() {
    let decoded = decoded_source_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register(IdentityLayer::Hir, &decoded).unwrap();
    assert!(matches!(
        pending.register(IdentityLayer::Mir, &decoded),
        Err(IdentityValidationError::DuplicateIdentity { .. })
    ));
    assert!(matches!(
        pending.finish(),
        Err(IdentityValidationError::Poisoned)
    ));
}

#[test]
fn unresolved_registration_cannot_commit() {
    let decoded = decoded_source_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register(IdentityLayer::Hir, &decoded).unwrap();
    assert!(matches!(
        pending.finish(),
        Err(IdentityValidationError::UnresolvedIdentity { .. })
    ));
}

#[test]
fn missing_reference_poisons_the_transaction() {
    let decoded = decoded_exact_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register(IdentityLayer::Hir, &decoded).unwrap();

    let error = pending.resolve(&decoded).unwrap_err();
    assert!(matches!(
        error,
        IdentityValidationError::InvalidRecord { ref reason, .. }
            if reason.contains("missing type identity")
    ));
    assert!(matches!(
        pending.finish(),
        Err(IdentityValidationError::Poisoned)
    ));
}

#[test]
fn earlier_layer_cannot_reference_a_future_layer_candidate() {
    let source = decoded_source_type();
    let exact = decoded_exact_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register(IdentityLayer::Mir, &source).unwrap();
    pending.register(IdentityLayer::Hir, &exact).unwrap();

    let error = pending.resolve(&exact).unwrap_err();
    assert!(matches!(
        error,
        IdentityValidationError::InvalidRecord { ref reason, .. }
            if reason.contains("HIR identity references future MIR type identity")
    ));
}

#[test]
fn registration_closes_when_resolution_starts() {
    let source = decoded_source_type();
    let exact = decoded_exact_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending.register(IdentityLayer::Hir, &source).unwrap();
    pending.resolve(&source).unwrap();

    assert_eq!(
        pending.register(IdentityLayer::Hir, &exact),
        Err(IdentityValidationError::RegistrationClosed)
    );
}

#[test]
fn registration_rejects_an_id_that_does_not_match_its_decoded_key() {
    let mut bytes = encode(&source_type_record()).unwrap();
    assert_eq!(&bytes[..4], &[0xa2, 1, 0x58, 0x20]);
    bytes[4] ^= 1;
    let decoded = decode_canonical::<
        DecodedCborIdentityRecord<PersistentTypeId, DecodedSourceDeclarationKey>,
    >(&bytes, DecodeLimits::default())
    .unwrap();
    let mut pending = PendingIdentityValidation::new();

    assert!(matches!(
        pending.register(IdentityLayer::Hir, &decoded),
        Err(IdentityValidationError::IdentityMismatch { .. })
    ));
}
