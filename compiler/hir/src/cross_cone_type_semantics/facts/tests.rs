use std::collections::BTreeMap;

use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    EnumVariantIdentityKey, ExactTypeKey, PackagePath, PersistentEnumVariantId,
    PersistentIdMismatch, PersistentTypeId, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind,
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

struct Shapes(BTreeMap<PersistentExactTypeId, ExactTypeFactShapeV1>);
impl ExactTypeFactsSemanticAuthority<&'static str> for Shapes {
    fn fact_shape(&self, id: PersistentExactTypeId) -> Result<&ExactTypeFactShapeV1, &'static str> {
        self.0.get(&id).ok_or("missing shape")
    }
}

#[test]
fn nested_zst_and_pointer_facts_are_replayed_from_semantic_shapes() {
    let unit = facts("Unit", ZstStatus::ZeroSized);
    let nested = facts("Nested", ZstStatus::ZeroSized);
    let pointer = facts("Pointer", ZstStatus::NonZero);
    let shapes = Shapes(BTreeMap::from([
        (unit.exact(), ExactTypeFactShapeV1::Unit),
        (
            nested.exact(),
            ExactTypeFactShapeV1::OrdinaryStruct {
                fields: vec![unit.exact(), unit.exact()],
            },
        ),
        (pointer.exact(), ExactTypeFactShapeV1::Pointer),
    ]));
    let table = CanonicalExactTypeFactsV1::try_new(vec![unit, nested, pointer]).unwrap();
    assert_eq!(
        table
            .validate_semantics(&shapes)
            .unwrap()
            .get(nested.exact()),
        Some(&nested)
    );
    let forged = CanonicalExactTypeFactsV1::try_new(vec![
        unit,
        facts("Nested", ZstStatus::NonZero),
        pointer,
    ])
    .unwrap();
    assert!(matches!(
        forged.validate_semantics(&shapes),
        Err(ExactTypeFactsSemanticError::Mismatch { .. })
    ));
}

#[test]
fn enum_is_nonzero_even_when_its_payload_is_empty() {
    let enumeration = facts("Flag", ZstStatus::NonZero);
    let shapes = Shapes(BTreeMap::from([(
        enumeration.exact(),
        ExactTypeFactShapeV1::Enum { variants: vec![] },
    )]));
    let table = CanonicalExactTypeFactsV1::try_new(vec![enumeration]).unwrap();
    table.validate_semantics(&shapes).unwrap();
    let forged =
        CanonicalExactTypeFactsV1::try_new(vec![facts("Flag", ZstStatus::ZeroSized)]).unwrap();
    assert!(matches!(
        forged.validate_semantics(&shapes),
        Err(ExactTypeFactsSemanticError::Mismatch { .. })
    ));
}

#[test]
fn by_value_cycles_missing_closure_and_zst_c_layout_fields_are_rejected() {
    let a = facts("A", ZstStatus::ZeroSized);
    let b = facts("B", ZstStatus::ZeroSized);
    let table = CanonicalExactTypeFactsV1::try_new(vec![a, b]).unwrap();
    let cycle = Shapes(BTreeMap::from([
        (
            a.exact(),
            ExactTypeFactShapeV1::OrdinaryStruct {
                fields: vec![b.exact()],
            },
        ),
        (
            b.exact(),
            ExactTypeFactShapeV1::OrdinaryStruct {
                fields: vec![a.exact()],
            },
        ),
    ]));
    assert!(matches!(
        table.validate_semantics(&cycle),
        Err(ExactTypeFactsSemanticError::ByValueCycle(_))
    ));
    let only_a = CanonicalExactTypeFactsV1::try_new(vec![a]).unwrap();
    assert!(matches!(
        only_a.validate_semantics(&cycle),
        Err(ExactTypeFactsSemanticError::MissingFacts(_))
    ));
    let c_layout = Shapes(BTreeMap::from([
        (
            a.exact(),
            ExactTypeFactShapeV1::CLayoutStruct {
                fields: vec![b.exact()],
            },
        ),
        (b.exact(), ExactTypeFactShapeV1::Unit),
    ]));
    assert!(matches!(
        table.validate_semantics(&c_layout),
        Err(ExactTypeFactsSemanticError::ZeroSizedCLayoutField { .. })
    ));
}

#[test]
fn enum_gc_is_the_and_of_variant_gc_and_reference_values_remain_managed() {
    let enum_key = declaration("Mixed", SourceNominalKind::Enum);
    let enum_exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&enum_key).unwrap(),
    ))
    .unwrap();
    let reference_key = declaration("Node", SourceNominalKind::Class);
    let reference_exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&reference_key).unwrap(),
    ))
    .unwrap();
    let variant_id = |name| {
        PersistentEnumVariantId::from_key(
            &EnumVariantIdentityKey::source(&enum_key, CanonicalIdentifier::new(name).unwrap())
                .unwrap(),
        )
        .unwrap()
    };
    let reference = ExactTypeFactsV1::try_new(
        reference_exact,
        ExactTypeKindV1::Reference,
        ExactTypeGcV1::ContainsManagedReferences,
    )
    .unwrap();
    let enumeration = ExactTypeFactsV1::try_new(
        enum_exact,
        ExactTypeKindV1::Value {
            zst: ZstStatus::NonZero,
        },
        ExactTypeGcV1::ContainsManagedReferences,
    )
    .unwrap();
    let empty = ExactEnumVariantFactsV1 {
        variant: variant_id("Empty"),
        fields: vec![],
        gc: ExactTypeGcV1::GcFree,
    };
    let payload = ExactEnumVariantFactsV1 {
        variant: variant_id("Payload"),
        fields: vec![reference_exact],
        gc: ExactTypeGcV1::ContainsManagedReferences,
    };
    let mut shapes = Shapes(BTreeMap::from([
        (reference_exact, ExactTypeFactShapeV1::Reference),
        (
            enum_exact,
            ExactTypeFactShapeV1::Enum {
                variants: vec![empty.clone(), payload.clone()],
            },
        ),
    ]));
    let table = CanonicalExactTypeFactsV1::try_new(vec![enumeration, reference]).unwrap();
    table.validate_semantics(&shapes).unwrap();
    let gc_free_enum =
        ExactTypeFactsV1::try_new(enum_exact, enumeration.kind(), ExactTypeGcV1::GcFree).unwrap();
    let forged = CanonicalExactTypeFactsV1::try_new(vec![gc_free_enum, reference]).unwrap();
    assert!(matches!(
        forged.validate_semantics(&shapes),
        Err(ExactTypeFactsSemanticError::Mismatch { .. })
    ));
    shapes.0.insert(
        enum_exact,
        ExactTypeFactShapeV1::Enum {
            variants: vec![
                empty,
                ExactEnumVariantFactsV1 {
                    gc: ExactTypeGcV1::GcFree,
                    ..payload
                },
            ],
        },
    );
    assert!(matches!(
        table.validate_semantics(&shapes),
        Err(ExactTypeFactsSemanticError::VariantGc { .. })
    ));
}
