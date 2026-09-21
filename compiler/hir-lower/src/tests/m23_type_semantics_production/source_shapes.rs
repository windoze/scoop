use super::*;
use scoop_identity::{DecodedPersistentId, PersistentEnumVariantId, PersistentIdResolver};
use scoop_wire::{decode_canonical, encode};

struct SourceIdentities {
    exacts: BTreeSet<PersistentExactTypeId>,
    variants: BTreeSet<PersistentEnumVariantId>,
}

impl SourceIdentities {
    fn from_hir(output: &hir::DependencyHirOutput) -> Self {
        let local = &output.output().local;
        let export = &output.output().export;
        let exacts = local
            .types
            .iter()
            .map(|(id, _)| local.exact_type_identities[id].id())
            .collect();
        let mut variants = BTreeSet::new();
        for (id, enumeration) in export.enums.iter() {
            for index in 0..enumeration.variants.len() {
                let reference =
                    hir::EnumVariantRef::checked(&export.enums, id, index as u32).unwrap();
                variants.insert(export.enum_member_identities[reference].id());
            }
        }
        Self { exacts, variants }
    }
}

impl PersistentIdResolver<PersistentExactTypeId> for SourceIdentities {
    type Error = &'static str;

    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        self.exacts
            .iter()
            .copied()
            .find(|known| decoded.verify(*known).is_ok())
            .ok_or("exact type absent from source HIR")
    }
}

impl PersistentIdResolver<PersistentEnumVariantId> for SourceIdentities {
    type Error = &'static str;

    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<PersistentEnumVariantId>,
    ) -> Result<PersistentEnumVariantId, Self::Error> {
        self.variants
            .iter()
            .copied()
            .find(|known| decoded.verify(*known).is_ok())
            .ok_or("enum variant absent from source HIR")
    }
}

#[test]
fn ordinary_source_fact_shapes_replay_after_byte_round_trip() {
    let output = lower_public_nominals();
    let public = public_interface(&output);
    let production = produce_cross_cone_type_semantics(&output, &public).unwrap();
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    let source = hir::CanonicalExactTypeFactShapesV1::try_new(
        production
            .foundation()
            .source_fact_shapes()
            .map(|(exact, shape)| hir::ExactTypeFactShapeRecordV1::new(exact, shape.clone()))
            .collect(),
        &mut meter,
    )
    .unwrap();
    let bytes = encode(&source).unwrap();
    let decoded: hir::DecodedCanonicalExactTypeFactShapesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let restored = decoded
        .resolve(&mut SourceIdentities::from_hir(&output), &mut meter)
        .unwrap();
    assert_eq!(source, restored);
    assert_eq!(source.records().len(), 7);
    let restored = FactShapes(
        restored
            .records()
            .iter()
            .map(|record| (record.exact(), record.shape().clone()))
            .collect(),
    );

    let dependency_shapes = FactShapes(
        production
            .dependency_facts()
            .iter()
            .map(|dependency| (dependency.exact, hir::ExactTypeFactShapeV1::Scalar))
            .collect(),
    );
    let dependency_table = hir::CanonicalExactTypeFactsV1::try_new(
        production
            .dependency_facts()
            .iter()
            .map(|dependency| {
                hir::ExactTypeFactsV1::try_new(
                    dependency.exact,
                    hir::ExactTypeKindV1::Value {
                        zst: hir::ZstStatus::NonZero,
                    },
                    hir::ExactTypeGcV1::GcFree,
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let checked_dependencies = DependencyFacts(
        dependency_table
            .validate_semantics(&dependency_shapes, &mut meter)
            .unwrap(),
    );
    production
        .section()
        .exact_facts()
        .validate_semantics_with_dependencies(&restored, &checked_dependencies, &mut meter)
        .unwrap();

    let mut mutated = production.section().exact_facts().records().to_vec();
    let target = mutated
        .iter_mut()
        .find(|record| record.exact() == word_value_exact(production.section()))
        .unwrap();
    *target = hir::ExactTypeFactsV1::try_new(
        target.exact(),
        hir::ExactTypeKindV1::Value {
            zst: hir::ZstStatus::ZeroSized,
        },
        target.gc(),
    )
    .unwrap();
    assert!(
        hir::CanonicalExactTypeFactsV1::try_new(mutated)
            .unwrap()
            .validate_semantics_with_dependencies(&restored, &checked_dependencies, &mut meter)
            .is_err()
    );
}
