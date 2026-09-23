use super::*;
use hir::{
    CanonicalNominalInheritanceInterfacesV1, NominalInheritanceEdgesV1,
    NominalInheritanceInterfaceV1,
};

pub(super) fn check(checked: CheckedSharedTypeFoundationV1<'_>) {
    checked
        .with_inheritance_graph(&[], &mut meter(), |graph, meter| {
            for record in checked.section().inheritance().records() {
                assert_eq!(graph.get(record.owner()).unwrap().edges(), record.edges());
            }
            for declaration in checked.metadata().public.nominal_interfaces().all_records() {
                graph
                    .replay_nominal_access(declaration.declaration(), meter)
                    .unwrap();
            }
        })
        .unwrap();

    let mut missing = checked.section().inheritance().records().to_vec();
    missing.pop().unwrap();
    assert!(
        matches!(reject(checked, missing), Error::InheritanceInventory(provider) if provider == checked.provider())
    );

    let mut modality = checked.section().inheritance().records().to_vec();
    let first = &modality[0];
    let owner = first.owner();
    let changed = if first.edges().modality() == hir::NominalInheritanceModalityV1::Open {
        hir::NominalInheritanceModalityV1::Final
    } else {
        hir::NominalInheritanceModalityV1::Open
    };
    modality[0] = NominalInheritanceInterfaceV1::try_new(
        NominalInheritanceEdgesV1::try_new(
            owner,
            changed,
            first.edges().direct_base(),
            first.edges().direct_interfaces().to_vec(),
        )
        .unwrap(),
        first.domains().clone(),
        first.constructors().clone(),
        first.slots().clone(),
        first.protected_members().clone(),
        first.slot_schemas().clone(),
    )
    .unwrap();
    assert!(
        matches!(reject(checked, modality), Error::InheritanceEdges(actual) if actual == owner)
    );

    let mut domains = checked.section().inheritance().records().to_vec();
    let first = &domains[0];
    domains[0] = NominalInheritanceInterfaceV1::try_new(
        first.edges().clone(),
        hir::NominalAccessDomainsV1::new(
            hir::PersistentLookupDomainV1::new(
                hir::PersistentAccessDomainV1::try_from_constraints(vec![
                    hir::PersistentAccessConstraintV1::Cone(ConeIdentity::SINGLE_FILE),
                ])
                .unwrap(),
            ),
            first.domains().inheritance().clone(),
            first.domains().slot().clone(),
        ),
        first.constructors().clone(),
        first.slots().clone(),
        first.protected_members().clone(),
        first.slot_schemas().clone(),
    )
    .unwrap();
    assert!(matches!(
        reject(checked, domains),
        Error::InheritanceDomains(_)
    ));
    for limits in [
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            owned_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(
            checked
                .with_inheritance_graph(&[], &mut BudgetMeter::new(limits), |_, _| ())
                .is_err()
        );
    }
}

fn reject(
    checked: CheckedSharedTypeFoundationV1<'_>,
    records: Vec<NominalInheritanceInterfaceV1>,
) -> Error {
    let source = checked.section();
    let candidate = CrossConeTypeSemanticsSectionV1::new(
        source.exact_facts().clone(),
        source.representation_support().clone(),
        CanonicalNominalInheritanceInterfacesV1::try_new(records).unwrap(),
        source.protected_declarations().clone(),
        source.protected_source_interfaces().clone(),
        source.protected_defaults().clone(),
        source.definition_sources().clone(),
        source.selected().clone(),
    );
    let foundation = candidate
        .validate_shared_foundation(checked.metadata(), &[], &mut meter())
        .unwrap();
    foundation
        .with_inheritance_graph(&[], &mut meter(), |_, _| ())
        .expect_err("invalid inheritance must fail against shared source declarations")
}
