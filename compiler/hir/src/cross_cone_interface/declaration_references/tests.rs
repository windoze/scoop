use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    PackagePath, PendingIdentityValidation, PersistentFunctionId, PersistentPropertyId,
    SourceDeclarationKey, SourceDeclarationSite, ValidatedIdentityGraph,
};
use scoop_wire::{DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;

#[test]
fn member_set_sorts_by_closed_kind_and_has_fixed_wire() {
    let fixture = fixture();
    let callable =
        PublicMemberRefV1::Callable(CallableDeclarationId::Function(fixture.function.id()));
    let property =
        PublicMemberRefV1::Property(PropertyDeclarationId::Property(fixture.property.id()));
    let members =
        CanonicalPublicMemberRefsV1::try_new(vec![property, callable]).expect("distinct members");

    assert_eq!(members.members(), &[callable, property]);
    assert_eq!(
        hex(&encode(&members).unwrap()),
        "82a2000101a20001015820a4f112f855c109889a4752eb27f637dc092ce1b21034b10cc1544a560a090850a2000201a20001015820d1e0e51d8c04d83e86e0e7e4998351129e016d00c659857c2c67a74192d48d13"
    );
}

#[test]
fn member_set_rejects_duplicate_inputs() {
    let fixture = fixture();
    let member =
        PublicMemberRefV1::Callable(CallableDeclarationId::Function(fixture.function.id()));

    assert_eq!(
        CanonicalPublicMemberRefsV1::try_new(vec![member, member]),
        Err(PublicMemberRefBuildError::Duplicate(member))
    );
}

#[test]
fn decoded_members_resolve_through_kind_specific_identity_authority() {
    let fixture = fixture();
    let expected = members(&fixture);
    let decoded = decode_members(&expected);
    let mut authority = authority(&fixture);

    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_member_order() {
    let fixture = fixture();
    let callable =
        PublicMemberRefV1::Callable(CallableDeclarationId::Function(fixture.function.id()));
    let property =
        PublicMemberRefV1::Property(PropertyDeclarationId::Property(fixture.property.id()));

    let mut duplicate_authority = authority(&fixture);
    let duplicate = decode_members(&MemberSequence(vec![callable, callable]));
    assert!(matches!(
        duplicate.resolve(&mut duplicate_authority),
        Err(PublicMemberRefSetValidationError::Duplicate {
            index: 1,
            member,
        }) if member == callable
    ));

    let mut reversed_authority = authority(&fixture);
    let reversed = decode_members(&MemberSequence(vec![property, callable]));
    assert!(matches!(
        reversed.resolve(&mut reversed_authority),
        Err(PublicMemberRefSetValidationError::NonCanonicalOrder { index: 1 })
    ));
}

#[test]
fn reader_rejects_unknown_outer_member_tags() {
    let error = decode_canonical::<DecodedPublicMemberRefV1>(
        &[0xa2, 0x00, 0x03, 0x01, 0xf6],
        DecodeLimits::default(),
    )
    .unwrap_err();

    assert!(matches!(
        error.kind(),
        scoop_wire::WireErrorKind::UnknownTag { tag: 3 }
    ));
}

struct Fixture {
    function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    property: CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey>,
}

fn fixture() -> Fixture {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    Fixture {
        function: CborIdentityRecord::from_key(SourceDeclarationKey::function(
            site.clone(),
            CanonicalIdentifier::new("invoke").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap(),
        property: CborIdentityRecord::from_key(SourceDeclarationKey::property(
            site,
            CanonicalIdentifier::new("value").unwrap(),
        ))
        .unwrap(),
    }
}

fn members(fixture: &Fixture) -> CanonicalPublicMemberRefsV1 {
    CanonicalPublicMemberRefsV1::try_new(vec![
        PublicMemberRefV1::Callable(CallableDeclarationId::Function(fixture.function.id())),
        PublicMemberRefV1::Property(PropertyDeclarationId::Property(fixture.property.id())),
    ])
    .unwrap()
}

fn authority(fixture: &Fixture) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_authority(ConeIdentity::SINGLE_FILE)
        .unwrap();
    pending
        .register_external_canonical_authority(fixture.function.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(fixture.property.clone())
        .unwrap();
    pending.finish().unwrap()
}

fn decode_members<T: WireEncode>(value: &T) -> DecodedCanonicalPublicMemberRefsV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

struct MemberSequence(Vec<PublicMemberRefV1>);

impl WireEncode for MemberSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for member in &self.0 {
            member.encode(encoder)?;
        }
        Ok(())
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
