use super::*;
use hir::{CanonicalSelectedExternalTypeUsesV1, SelectedExternalTypeUseV1, SelectedTypeUseV1};

pub(super) fn check(
    current: CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[CheckedSharedTypeFoundationV1<'_>],
) {
    current
        .validate_materialized_type_uses(dependencies, &mut meter())
        .unwrap();
    let original = current.section().selected().records();
    for position in 0..original.len() {
        let mut records = original.to_vec();
        records.remove(position);
        reject(current, dependencies, records);
    }
    let mut local_as_external = original.to_vec();
    local_as_external.push(SelectedExternalTypeUseV1::new(
        current.provider(),
        SelectedTypeUseV1::Representation {
            exact: current.facts().records()[0].exact(),
        },
    ));
    reject(current, dependencies, local_as_external);
    if let Some(first) = original.first() {
        let mut wrong_provider = original.to_vec();
        wrong_provider[0] = SelectedExternalTypeUseV1::new(current.provider(), first.usage());
        reject(current, dependencies, wrong_provider);
    }
}

fn reject(
    current: CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[CheckedSharedTypeFoundationV1<'_>],
    records: Vec<SelectedExternalTypeUseV1>,
) {
    let original = current.section();
    let candidate = CrossConeTypeSemanticsSectionV1::new(
        original.exact_facts().clone(),
        original.representation_support().clone(),
        original.inheritance().clone(),
        original.protected_declarations().clone(),
        original.protected_source_interfaces().clone(),
        original.protected_defaults().clone(),
        original.definition_sources().clone(),
        CanonicalSelectedExternalTypeUsesV1::try_new(records).unwrap(),
    );
    let candidate = candidate
        .validate_shared_foundation(current.metadata(), dependencies, &mut meter())
        .unwrap();
    assert!(matches!(
        candidate.validate_materialized_type_uses(dependencies, &mut meter()),
        Err(Error::TypeUseInventory)
    ));
}
