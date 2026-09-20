use super::source_inventory::identity_closure;
use super::*;
use scoop_wire::{decode_canonical, encode};

fn fixture() -> (
    hir::OrdinaryHirOutput<'static>,
    hir::CrossConeTypeSemanticsProductionV1,
) {
    let output = lower_public_nominals();
    let public = public_interface(&output);
    let production = produce_cross_cone_type_semantics(&output, &public).unwrap();
    (output, production)
}

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn source_foundation_product_round_trips_real_hir_and_generated_object_roles() {
    let (output, production) = fixture();
    let source = production
        .foundation()
        .source_transcript(&mut meter())
        .unwrap();
    let independent = hir::CrossConeTypeSemanticsFoundationV1::from_ordinary_hir(&output)
        .unwrap()
        .source_transcript(&mut meter())
        .unwrap();
    assert_eq!(source, independent);
    assert_eq!(source.entries().representations.records().len(), 7);
    assert_eq!(source.entries().source_roots.values().len(), 8);
    assert!(!source.entries().generated_nominals.is_empty());
    assert!(!source.entries().definition_sources.is_empty());
    let bytes = encode(&source).unwrap();
    let decoded: hir::DecodedTypeFoundationSourceAuthorityV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let legacy = hir::CanonicalHirFoundation::from_ordinary_output(&output).unwrap();
    let mut legacy_graph =
        super::source_inventory::identity_closure_for_foundation(&output, legacy);
    assert!(matches!(
        decoded.clone().resolve(&mut legacy_graph, &mut meter()),
        Err(hir::TypeFoundationSourceError::Constituent { field: 2, .. })
    ));
    let restored = decoded
        .resolve(&mut identity_closure(&output), &mut meter())
        .unwrap();
    assert_eq!(restored, source);
    restored
        .entries()
        .representations
        .validate_source_semantics(production.foundation(), &mut meter(), &WirePath::root())
        .unwrap();
}

#[test]
fn foundation_product_rejects_omitted_source_fact_representation_and_inheritance_inventory() {
    let (_, production) = fixture();
    let source = production
        .foundation()
        .source_transcript(&mut meter())
        .unwrap();
    for field in [8, 9, 11, 13] {
        let mut entries = source.clone().into_entries();
        match field {
            8 => entries.source_roots = hir::CanonicalSourceNominalIdsV1::default(),
            9 => entries.local_exact_facts = hir::CanonicalPersistentIdsV1::empty(),
            11 => {
                entries.local_inheritance_edges = hir::CanonicalNominalInheritanceEdgesV1::default()
            }
            13 => entries.representation_owners = hir::CanonicalPersistentIdsV1::empty(),
            _ => unreachable!(),
        }
        assert!(
            matches!(hir::TypeFoundationSourceAuthorityV1::try_new(entries, &mut meter()), Err(hir::TypeFoundationSourceError::Inventory { field: actual }) if actual == field)
        );
    }
}

#[test]
fn foundation_product_rejects_fact_ownership_overlap_and_different_access_snapshots() {
    let (_, production) = fixture();
    let source = production
        .foundation()
        .source_transcript(&mut meter())
        .unwrap();
    let mut entries = source.clone().into_entries();
    let exact = entries.local_exact_facts.values()[0];
    entries.dependency_facts = hir::CanonicalTypeSectionDependencyFactsV1::try_new(
        vec![hir::TypeSectionDependencyFactV1 {
            provider: ConeIdentity::CORE,
            exact,
        }],
        &mut meter(),
    )
    .unwrap();
    assert!(
        matches!(hir::TypeFoundationSourceAuthorityV1::try_new(entries, &mut meter()), Err(hir::TypeFoundationSourceError::FactOwnershipOverlap(actual)) if actual == exact)
    );

    let mut entries = source.into_entries();
    let owner = entries.representations.records()[0].owner();
    entries.sources = hir::CanonicalTypeSourceNominalsV1::try_new(
        entries
            .sources
            .records()
            .iter()
            .map(|record| {
                if record.owner() != hir::SourceNominalId::Concrete(owner) {
                    return record.clone();
                }
                hir::TypeSourceNominalV1::new(
                    record.owner(),
                    hir::DeclarationAccessSourceV1::try_new(
                        hir::DeclaredVisibilityV1::Private,
                        record.access().lexical_owners().to_vec(),
                        record.access().definition_origin().clone(),
                    )
                    .unwrap(),
                )
            })
            .collect(),
        &mut meter(),
    )
    .unwrap();
    assert!(
        matches!(hir::TypeFoundationSourceAuthorityV1::try_new(entries, &mut meter()), Err(hir::TypeFoundationSourceError::RepresentationAccess(actual)) if actual == owner)
    );
}
