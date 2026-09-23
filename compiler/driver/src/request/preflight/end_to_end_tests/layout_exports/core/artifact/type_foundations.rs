//! Mutations retain the declarations decoded from the actual artifact bytes.

use super::*;
use hir::{
    CanonicalExactTypeFactsV1, CanonicalNominalRepresentationSupportV1,
    CheckedSharedTypeFoundationV1, CrossConeTypeSemanticsSectionV1, ExactTypeFactsV1,
    ExactTypeGcV1, ExactTypeKindV1, NominalRepresentationShapeV1, NominalRepresentationSupportV1,
    SharedTypeMetadataError as Error, ZstStatus,
};
use scoop_identity::{
    ExactTypeKey, NonEmptyVec, PersistentExactTypeId, PersistentTypeId, SourceDeclarationKey,
};

pub(super) fn check(checked: CheckedSharedTypeFoundationV1<'_>) {
    facts(checked);
    representations(checked);
    let mut exhausted = scoop_wire::BudgetMeter::new(DecodeLimits {
        validation_work_units: 0,
        ..DecodeLimits::default()
    });
    assert!(
        checked
            .section()
            .validate_shared_foundation(checked.metadata(), &[], &mut exhausted)
            .is_err()
    );
}

fn facts(checked: CheckedSharedTypeFoundationV1<'_>) {
    let facts = checked.facts();
    let records = facts.records();
    let mut missing = records.to_vec();
    missing.pop().unwrap();
    assert!(matches!(
        reject_facts(checked, missing),
        Error::MissingFact(_) | Error::FactInventory
    ));

    let extra = PersistentExactTypeId::from_key(&ExactTypeKey::Tuple(
        NonEmptyVec::new(vec![records[0].exact(); 3]).unwrap(),
    ))
    .unwrap();
    assert!(checked.facts().get(extra).is_none());
    let mut surplus = records.to_vec();
    surplus.push(ExactTypeFactsV1::try_new(extra, records[0].kind(), records[0].gc()).unwrap());
    assert!(matches!(
        reject_facts(checked, surplus),
        Error::FactInventory
    ));

    let mut wrong_gc = records.to_vec();
    let scalar = wrong_gc
        .iter_mut()
        .find(|fact| {
            fact.gc() == ExactTypeGcV1::GcFree
                && fact.kind()
                    == (ExactTypeKindV1::Value {
                        zst: ZstStatus::NonZero,
                    })
        })
        .expect("the core fixture includes scalar value facts");
    *scalar = ExactTypeFactsV1::try_new(
        scalar.exact(),
        scalar.kind(),
        ExactTypeGcV1::ContainsManagedReferences,
    )
    .unwrap();
    assert!(matches!(reject_facts(checked, wrong_gc), Error::Facts(_)));
}

fn reject_facts(
    checked: CheckedSharedTypeFoundationV1<'_>,
    records: Vec<ExactTypeFactsV1>,
) -> Error {
    let section = replacement(
        checked,
        CanonicalExactTypeFactsV1::try_new(records).unwrap(),
        checked.representations().table().clone(),
    );
    reject(checked, &section)
}

fn representations(checked: CheckedSharedTypeFoundationV1<'_>) {
    let records = checked.representations().table().records();
    let mut missing = records.to_vec();
    missing.pop().unwrap();
    assert!(matches!(
        reject_representations(checked, missing),
        Error::RepresentationInventory
    ));

    let mut omitted_field = records.to_vec();
    let record = omitted_field.iter_mut().find(|record| {
        matches!(record.shape(), NominalRepresentationShapeV1::Struct { fields, .. } if fields.len() > 1)
    }).expect("the core fixture includes SourceLocation with multiple fields");
    let owner = record.owner();
    let mut shape = record.shape().clone();
    let NominalRepresentationShapeV1::Struct { fields, .. } = &mut shape else {
        unreachable!("selected a struct above");
    };
    fields.pop().unwrap();
    let key = checked
        .metadata()
        .identities
        .canonical_key::<PersistentTypeId, SourceDeclarationKey>(owner)
        .unwrap();
    *record =
        NominalRepresentationSupportV1::try_new(&key, record.declaration_access().clone(), shape)
            .unwrap();
    assert!(
        matches!(reject_representations(checked, omitted_field), Error::Representation(actual) if actual == owner)
    );

    let mut wrong_source = records.to_vec();
    let record = &mut wrong_source[0];
    let owner = record.owner();
    let access = record.declaration_access();
    let origin = records
        .iter()
        .map(|other| other.declaration_access().definition_origin())
        .find(|origin| *origin != access.definition_origin())
        .expect("the core fixture contains distinct declaration locations");
    let access = hir::DeclarationAccessSourceV1::try_new(
        access.declared_visibility(),
        access.lexical_owners().to_vec(),
        origin.clone(),
    )
    .unwrap();
    let key = checked
        .metadata()
        .identities
        .canonical_key::<PersistentTypeId, SourceDeclarationKey>(owner)
        .unwrap();
    *record =
        NominalRepresentationSupportV1::try_new(&key, access, record.shape().clone()).unwrap();
    assert!(
        matches!(reject_representations(checked, wrong_source), Error::Representation(actual) if actual == owner)
    );
}

fn reject_representations(
    checked: CheckedSharedTypeFoundationV1<'_>,
    records: Vec<NominalRepresentationSupportV1>,
) -> Error {
    let section = replacement(
        checked,
        checked.section().exact_facts().clone(),
        CanonicalNominalRepresentationSupportV1::try_new(records).unwrap(),
    );
    reject(checked, &section)
}

fn replacement(
    checked: CheckedSharedTypeFoundationV1<'_>,
    facts: CanonicalExactTypeFactsV1,
    representations: CanonicalNominalRepresentationSupportV1,
) -> CrossConeTypeSemanticsSectionV1 {
    let section = checked.section();
    CrossConeTypeSemanticsSectionV1::new(
        facts,
        representations,
        section.inheritance().clone(),
        section.protected_declarations().clone(),
        section.protected_source_interfaces().clone(),
        section.protected_defaults().clone(),
        section.definition_sources().clone(),
        section.selected().clone(),
    )
}

fn reject(
    checked: CheckedSharedTypeFoundationV1<'_>,
    candidate: &CrossConeTypeSemanticsSectionV1,
) -> Error {
    candidate
        .validate_shared_foundation(checked.metadata(), &[], &mut meter())
        .err()
        .expect("mutated type foundation must fail against the original declarations")
}
