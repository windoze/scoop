use super::*;
use crate::{
    CanonicalExactTypeFactsV1, ExactTypeFactsSemanticAuthority, ExactTypeFactsV1, ExactTypeGcV1,
    ExactTypeKindV1, ZstStatus,
};
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DeclarationScope, DecodedPersistentId,
    DefinitionOwnerChain, EnumVariantIdentityKey, ExactTypeKey, PackagePath, PersistentIdResolver,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

fn unit() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap()
}

fn any() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Any.identity_record().id(),
    ))
    .unwrap()
}

fn variant(name: &str) -> PersistentEnumVariantId {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let owner = SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new("Choice").unwrap(),
        SourceNominalKind::Enum,
        0,
    );
    PersistentEnumVariantId::from_key(
        &EnumVariantIdentityKey::source(&owner, CanonicalIdentifier::new(name).unwrap()).unwrap(),
    )
    .unwrap()
}

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[derive(Default)]
struct Resolver {
    queries: usize,
}

impl PersistentIdResolver<PersistentExactTypeId> for Resolver {
    type Error = &'static str;
    fn resolve(
        &mut self,
        value: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        self.queries += 1;
        [unit(), any()]
            .into_iter()
            .find(|known| value.verify(*known).is_ok())
            .ok_or("unknown exact type")
    }
}
impl PersistentIdResolver<PersistentEnumVariantId> for Resolver {
    type Error = &'static str;
    fn resolve(
        &mut self,
        value: DecodedPersistentId<PersistentEnumVariantId>,
    ) -> Result<PersistentEnumVariantId, Self::Error> {
        self.queries += 1;
        [variant("First"), variant("Second")]
            .into_iter()
            .find(|known| value.verify(*known).is_ok())
            .ok_or("unknown enum variant")
    }
}

fn round_trip(shape: ExactTypeFactShapeV1, expected: Vec<u8>) {
    assert_eq!(encode(&shape).unwrap(), expected);
    let decoded: DecodedExactTypeFactShapeV1 =
        decode_canonical(&expected, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), expected);
    assert_eq!(
        decoded
            .resolve(&mut Resolver::default(), &mut meter())
            .unwrap(),
        shape
    );
}

#[test]
fn all_eight_source_shape_tags_have_fixed_wire_vectors() {
    for (tag, shape) in [
        (1, ExactTypeFactShapeV1::Unit),
        (2, ExactTypeFactShapeV1::Scalar),
        (3, ExactTypeFactShapeV1::Pointer),
        (4, ExactTypeFactShapeV1::Reference),
    ] {
        round_trip(shape, vec![0xa1, 0, tag]);
    }
    for (tag, shape) in [
        (
            5,
            ExactTypeFactShapeV1::OrdinaryStruct {
                fields: vec![unit(), unit()],
            },
        ),
        (
            6,
            ExactTypeFactShapeV1::CLayoutStruct {
                fields: vec![unit(), unit()],
            },
        ),
        (
            7,
            ExactTypeFactShapeV1::Tuple {
                elements: vec![unit(), unit()],
            },
        ),
    ] {
        round_trip(
            shape,
            [
                vec![0xa2, 0, tag, 1, 0x82],
                encode(&unit()).unwrap(),
                encode(&unit()).unwrap(),
            ]
            .concat(),
        );
    }
    let variant = ExactEnumVariantFactsV1 {
        variant: variant("First"),
        fields: vec![any(), unit()],
        gc: ExactTypeGcV1::ContainsManagedReferences,
    };
    let expected = [
        vec![0xa2, 0, 8, 1, 0x81, 0xa3, 1],
        encode(&variant.variant).unwrap(),
        vec![2, 0x82],
        encode(&any()).unwrap(),
        encode(&unit()).unwrap(),
        vec![3, 0xa1, 0, 2],
    ]
    .concat();
    round_trip(
        ExactTypeFactShapeV1::Enum {
            variants: vec![variant],
        },
        expected,
    );
}

#[test]
fn shape_tables_sort_only_the_exact_keys_and_preserve_source_sequences() {
    let mut variants = vec![
        ExactEnumVariantFactsV1 {
            variant: variant("First"),
            fields: vec![any(), unit(), any()],
            gc: ExactTypeGcV1::ContainsManagedReferences,
        },
        ExactEnumVariantFactsV1 {
            variant: variant("Second"),
            fields: vec![],
            gc: ExactTypeGcV1::GcFree,
        },
    ];
    variants.sort_unstable_by_key(|item| std::cmp::Reverse(item.variant));
    let table = CanonicalExactTypeFactShapesV1::try_new(
        vec![
            ExactTypeFactShapeRecordV1::new(
                any(),
                ExactTypeFactShapeV1::Enum {
                    variants: variants.clone(),
                },
            ),
            ExactTypeFactShapeRecordV1::new(unit(), ExactTypeFactShapeV1::Unit),
        ],
        &mut meter(),
    )
    .unwrap();
    let bytes = encode(&table).unwrap();
    let decoded: DecodedCanonicalExactTypeFactShapesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let resolved = decoded
        .resolve(&mut Resolver::default(), &mut meter())
        .unwrap();
    assert_eq!(resolved, table);
    assert_eq!(
        resolved.get(any()),
        Some(&ExactTypeFactShapeV1::Enum { variants })
    );
    assert!(
        resolved
            .records()
            .windows(2)
            .all(|pair| pair[0].exact() < pair[1].exact())
    );

    let mut reversed = table.records().to_vec();
    reversed.reverse();
    let bytes = [
        vec![0x82],
        encode(&reversed[0]).unwrap(),
        encode(&reversed[1]).unwrap(),
    ]
    .concat();
    let decoded: DecodedCanonicalExactTypeFactShapesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut Resolver::default(), &mut meter()),
        Err(TypeFactShapeSourceError::NonCanonicalOrder(_))
    ));
}

#[test]
fn duplicate_records_variants_and_empty_tuples_are_rejected() {
    let record = ExactTypeFactShapeRecordV1::new(unit(), ExactTypeFactShapeV1::Unit);
    let bytes = [
        vec![0x82],
        encode(&record).unwrap(),
        encode(&record).unwrap(),
    ]
    .concat();
    let decoded: DecodedCanonicalExactTypeFactShapesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut Resolver::default(), &mut meter()),
        Err(TypeFactShapeSourceError::NonCanonicalOrder(_))
    ));
    assert!(matches!(
        CanonicalExactTypeFactShapesV1::try_new(vec![record.clone(), record], &mut meter()),
        Err(TypeFactShapeSourceError::NonCanonicalOrder(_))
    ));
    let variant = ExactEnumVariantFactsV1 {
        variant: variant("First"),
        fields: vec![],
        gc: ExactTypeGcV1::GcFree,
    };
    for shape in [
        ExactTypeFactShapeV1::Tuple { elements: vec![] },
        ExactTypeFactShapeV1::Enum {
            variants: vec![variant.clone(), variant],
        },
    ] {
        assert!(
            CanonicalExactTypeFactShapesV1::try_new(
                vec![ExactTypeFactShapeRecordV1::new(unit(), shape.clone())],
                &mut meter()
            )
            .is_err()
        );
        let decoded: DecodedExactTypeFactShapeV1 =
            decode_canonical(&encode(&shape).unwrap(), DecodeLimits::default()).unwrap();
        assert!(
            decoded
                .resolve(&mut Resolver::default(), &mut meter())
                .is_err()
        );
    }
}

#[test]
fn closed_wire_products_reject_unknown_tags_extra_fields_and_wrong_id_widths() {
    for bytes in [
        vec![0xa1, 0, 9],
        vec![0xa2, 0, 1, 1, 0],
        vec![0xa1, 0, 5],
        vec![0xa2, 0, 7, 1, 0x81, 0x41, 1],
        vec![0xa2, 0, 8, 1, 0x81, 0xa0],
    ] {
        assert!(
            decode_canonical::<DecodedExactTypeFactShapeV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }
}

#[test]
fn unknown_field_and_variant_identities_cannot_become_source_evidence() {
    let unknown = PersistentExactTypeId::from_key(&ExactTypeKey::RawPointer(unit())).unwrap();
    for shape in [
        ExactTypeFactShapeV1::OrdinaryStruct {
            fields: vec![unknown],
        },
        ExactTypeFactShapeV1::Enum {
            variants: vec![ExactEnumVariantFactsV1 {
                variant: variant("Unknown"),
                fields: vec![],
                gc: ExactTypeGcV1::GcFree,
            }],
        },
    ] {
        let decoded: DecodedExactTypeFactShapeV1 =
            decode_canonical(&encode(&shape).unwrap(), DecodeLimits::default()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut Resolver::default(), &mut meter()),
            Err(TypeFactShapeSourceError::Reference(_))
        ));
    }
}

#[test]
fn resolution_checks_shared_budget_before_allocating_or_querying_refs() {
    let shape = ExactTypeFactShapeV1::OrdinaryStruct {
        fields: vec![unit(), any()],
    };
    let decoded: DecodedExactTypeFactShapeV1 =
        decode_canonical(&encode(&shape).unwrap(), DecodeLimits::default()).unwrap();
    for limits in [
        DecodeLimits {
            semantic_table_entries: 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
    ] {
        let mut resolver = Resolver::default();
        assert!(matches!(
            decoded
                .clone()
                .resolve(&mut resolver, &mut BudgetMeter::new(limits)),
            Err(TypeFactShapeSourceError::Resource(_))
        ));
        assert_eq!(resolver.queries, 0);
    }
}

struct Source(CanonicalExactTypeFactShapesV1);
impl ExactTypeFactsSemanticAuthority<&'static str> for Source {
    fn fact_shape(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ExactTypeFactShapeV1, &'static str> {
        self.0.get(exact).ok_or("missing source fact shape")
    }
}

#[test]
fn byte_restored_source_shapes_reject_a_forged_zst_conclusion() {
    let source = CanonicalExactTypeFactShapesV1::try_new(
        vec![ExactTypeFactShapeRecordV1::new(
            unit(),
            ExactTypeFactShapeV1::Unit,
        )],
        &mut meter(),
    )
    .unwrap();
    let decoded: DecodedCanonicalExactTypeFactShapesV1 =
        decode_canonical(&encode(&source).unwrap(), DecodeLimits::default()).unwrap();
    let source = Source(
        decoded
            .resolve(&mut Resolver::default(), &mut meter())
            .unwrap(),
    );
    for (zst, accepted) in [(ZstStatus::ZeroSized, true), (ZstStatus::NonZero, false)] {
        let candidate = CanonicalExactTypeFactsV1::try_new(vec![
            ExactTypeFactsV1::try_new(
                unit(),
                ExactTypeKindV1::Value { zst },
                ExactTypeGcV1::GcFree,
            )
            .unwrap(),
        ])
        .unwrap();
        assert_eq!(
            candidate.validate_semantics(&source, &mut meter()).is_ok(),
            accepted
        );
    }
}
