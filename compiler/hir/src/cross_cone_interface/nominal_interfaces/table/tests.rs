use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    NominalDeclarationOwner, PackagePath, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind, ValidatedIdentityGraph,
};
use scoop_wire::{DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::{
    CanonicalBinderListV1, CanonicalPersistentIdsV1, CanonicalPublicMemberRefsV1,
    CanonicalSignatureTypesV1, NominalSourceShapeV1, PublicNominalKindV1,
};

#[test]
fn producer_sorts_records_rejects_duplicates_and_has_stable_wire() {
    let first = fixture("First");
    let second = fixture("Second");
    let interfaces =
        CanonicalNominalInterfacesV1::try_new(vec![second.record.clone(), first.record.clone()])
            .unwrap();

    assert!(interfaces.records()[0].declaration() < interfaces.records()[1].declaration());
    assert_eq!(
        interfaces.get(first.record.declaration()),
        Some(&first.record)
    );
    assert!(!interfaces.is_empty());
    assert_eq!(
        CanonicalNominalInterfacesV1::try_new(vec![first.record.clone(), first.record.clone(),]),
        Err(NominalInterfaceSetBuildError::DuplicateDeclaration(
            first.record.declaration()
        ))
    );

    let expected = [
        b"\x82".as_slice(),
        encode(&interfaces.records()[0]).unwrap().as_slice(),
        encode(&interfaces.records()[1]).unwrap().as_slice(),
    ]
    .concat();
    assert_eq!(encode(&interfaces).unwrap(), expected);
}

#[test]
fn decoded_table_resolves_records_through_typed_authority() {
    let first = fixture("First");
    let second = fixture("Second");
    let expected =
        CanonicalNominalInterfacesV1::try_new(vec![first.record.clone(), second.record.clone()])
            .unwrap();
    let decoded = decode_table(&expected);
    let mut authority = authority(&[first.identity, second.identity]);

    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_record_order() {
    let first = fixture("First");
    let second = fixture("Second");
    let (low, high) = if first.record.declaration() < second.record.declaration() {
        (&first, &second)
    } else {
        (&second, &first)
    };

    let duplicate = decode_table(&RecordSequence(vec![
        low.record.clone(),
        low.record.clone(),
    ]));
    let mut duplicate_authority = authority(&[first.identity.clone(), second.identity.clone()]);
    assert!(matches!(
        duplicate.resolve(&mut duplicate_authority),
        Err(NominalInterfaceSetValidationError::DuplicateDeclaration { index: 1, .. })
    ));

    let reversed = decode_table(&RecordSequence(vec![
        high.record.clone(),
        low.record.clone(),
    ]));
    let mut reversed_authority = authority(&[first.identity, second.identity]);
    assert_eq!(
        reversed
            .resolve(&mut reversed_authority)
            .unwrap_err()
            .to_string(),
        "non-canonical nominal interface order at index 1"
    );
}

struct Fixture {
    identity: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    record: NominalInterfaceRecordV1,
}

fn fixture(name: &str) -> Fixture {
    let key = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let identity = CborIdentityRecord::from_key(key).unwrap();
    let record = NominalInterfaceRecordV1::try_new(
        NominalDeclarationOwner::Concrete(identity.id()),
        PublicNominalKindV1::Class,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        NominalSourceShapeV1::Class,
    )
    .unwrap();
    Fixture { identity, record }
}

fn authority(
    identities: &[CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>],
) -> ValidatedIdentityGraph {
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    for identity in identities {
        pending
            .register_external_canonical_authority(identity.clone())
            .unwrap();
    }
    pending.finish().unwrap()
}

fn decode_table<T: WireEncode>(value: &T) -> DecodedCanonicalNominalInterfacesV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

struct RecordSequence(Vec<NominalInterfaceRecordV1>);

impl WireEncode for RecordSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}
