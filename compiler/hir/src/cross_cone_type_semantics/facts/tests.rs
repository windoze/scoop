use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    PackagePath, PersistentIdMismatch, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{decode_canonical, encode};

use super::*;

fn exact(name: &str) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&declaration(name, SourceNominalKind::Struct))
            .unwrap(),
    ))
    .unwrap()
}

fn declaration(name: &str, kind: SourceNominalKind) -> SourceDeclarationKey {
    let site = SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    SourceDeclarationKey::nominal(site, CanonicalIdentifier::new(name).unwrap(), kind, 0)
}

fn facts(name: &str, zst: ZstStatus) -> ExactTypeFactsV1 {
    ExactTypeFactsV1::try_new(
        exact(name),
        ExactTypeKindV1::Value { zst },
        ExactTypeGcV1::GcFree,
    )
    .unwrap()
}

struct Resolver(PersistentExactTypeId);
impl PersistentIdResolver<PersistentExactTypeId> for Resolver {
    type Error = PersistentIdMismatch<PersistentExactTypeId>;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        id.verify(self.0)
    }
}

#[test]
fn fact_wire_has_fixed_closed_products_and_typed_identity() {
    let record = facts("Empty", ZstStatus::ZeroSized);
    let expected = [
        b"\xa3\x01\x58\x20".as_slice(),
        record.exact().as_array(),
        b"\x02\xa2\x00\x01\x01\xa1\x00\x01\x03\xa1\x00\x01",
    ]
    .concat();
    assert_eq!(encode(&record).unwrap(), expected);
    let decoded: DecodedExactTypeFactsV1 = decode_canonical(&expected).unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver(record.exact())).unwrap(),
        record
    );
    assert!(decoded.resolve(&mut Resolver(exact("Other"))).is_err());
    assert_eq!(encode(&ExactTypeKindV1::Reference).unwrap(), [0xa1, 0, 2]);
    assert_eq!(encode(&ZstStatus::NonZero).unwrap(), [0xa1, 0, 2]);
}

#[test]
fn impossible_gc_combinations_cannot_form_resolved_facts() {
    assert_eq!(
        ExactTypeFactsV1::try_new(
            exact("Any"),
            ExactTypeKindV1::Reference,
            ExactTypeGcV1::GcFree
        ),
        Err(ExactTypeFactsBuildError::GcFreeReference)
    );
    assert_eq!(
        ExactTypeFactsV1::try_new(
            exact("Empty"),
            ExactTypeKindV1::Value {
                zst: ZstStatus::ZeroSized
            },
            ExactTypeGcV1::ContainsManagedReferences
        ),
        Err(ExactTypeFactsBuildError::ManagedZeroSizedValue)
    );
    for bytes in [
        &[0xa1, 0, 3][..],
        &[0xa2, 0, 2, 1, 0][..],
        &[0xa1, 1, 1][..],
    ] {
        assert!(decode_canonical::<ExactTypeKindV1>(bytes).is_err());
    }
}

#[test]
fn facts_table_canonicalizes_production_and_rejects_duplicate_wire() {
    let record = facts("Empty", ZstStatus::ZeroSized);
    assert!(CanonicalExactTypeFactsV1::try_new(vec![record, record]).is_err());
    let bytes = [
        b"\x82".as_slice(),
        &encode(&record).unwrap(),
        &encode(&record).unwrap(),
    ]
    .concat();
    let decoded: DecodedCanonicalExactTypeFactsV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.resolve(&mut Resolver(record.exact())),
        Err(ExactTypeFactsTableResolutionError::Order(_))
    ));
    assert_eq!(
        encode(&CanonicalExactTypeFactsV1::default()).unwrap(),
        [0x80]
    );
}
