use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    IdentityReferenceError, PackagePath, PendingIdentityValidation, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, ValidatedIdentityGraph,
};
use scoop_wire::{Encoder, WireEncode, decode_canonical, encode};

use super::*;

#[test]
fn producer_sorts_typed_ids_and_rejects_duplicates() {
    let fixture = fixture();
    let mut expected = vec![fixture.first.id(), fixture.second.id()];
    expected.sort_unstable();

    let ids = CanonicalPersistentIdsV1::try_new(expected.iter().copied().rev().collect()).unwrap();
    assert_eq!(ids.values(), expected);

    assert_eq!(
        CanonicalPersistentIdsV1::try_new(vec![expected[0], expected[0]]),
        Err(CanonicalPersistentIdSetBuildError::Duplicate(expected[0]))
    );
}

#[test]
fn typed_id_set_has_fixed_wire() {
    let fixture = fixture();
    let ids = CanonicalPersistentIdsV1::try_new(vec![fixture.first.id()]).unwrap();

    assert_eq!(
        encode(&ids).unwrap(),
        [b"\x81\x58\x20".as_slice(), fixture.first.id().as_array()].concat()
    );
}

#[test]
fn decoded_set_resolves_through_kind_specific_authority() {
    let fixture = fixture();
    let expected =
        CanonicalPersistentIdsV1::try_new(vec![fixture.second.id(), fixture.first.id()]).unwrap();
    let decoded = decode_ids(&expected);
    let mut authority = authority(&fixture);

    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_id_order() {
    let fixture = fixture();
    let mut ordered = vec![fixture.first.id(), fixture.second.id()];
    ordered.sort_unstable();

    let duplicate = IdSequence(vec![ordered[0], ordered[0]]);
    let mut duplicate_authority = authority(&fixture);
    assert!(matches!(
        decode_ids(&duplicate).resolve(&mut duplicate_authority),
        Err(CanonicalPersistentIdSetValidationError::Duplicate {
            index: 1,
            id,
        }) if id == ordered[0]
    ));

    let reversed = IdSequence(ordered.into_iter().rev().collect());
    let mut reversed_authority = authority(&fixture);
    assert!(matches!(
        decode_ids(&reversed).resolve(&mut reversed_authority),
        Err(CanonicalPersistentIdSetValidationError::NonCanonicalOrder { index: 1 })
    ));
}

#[test]
fn reader_rejects_missing_typed_id_authority() {
    let fixture = fixture();
    let ids = CanonicalPersistentIdsV1::try_new(vec![fixture.first.id()]).unwrap();
    let decoded = decode_ids(&ids);
    let mut authority = PendingIdentityValidation::new().finish().unwrap();

    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(CanonicalPersistentIdSetValidationError::Reference {
            index: 0,
            error: IdentityReferenceError::Missing { .. },
        })
    ));
}

struct Fixture {
    first: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    second: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
}

fn fixture() -> Fixture {
    Fixture {
        first: nominal("First"),
        second: nominal("Second"),
    }
}

fn nominal(name: &str) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap()
}

fn authority(fixture: &Fixture) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_canonical_authority(fixture.first.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(fixture.second.clone())
        .unwrap();
    pending.finish().unwrap()
}

fn decode_ids<T: WireEncode>(value: &T) -> DecodedCanonicalPersistentIdsV1<PersistentTypeId> {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

struct IdSequence(Vec<PersistentTypeId>);

impl WireEncode for IdSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for id in &self.0 {
            id.encode(encoder)?;
        }
        Ok(())
    }
}
