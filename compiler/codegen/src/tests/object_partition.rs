use std::collections::BTreeSet;

use super::*;

#[test]
fn partitions_each_callable_body_away_from_non_callable_definitions() {
    let mut module = exceptions_module();
    module.output = scoop_lir::LirOutput::Library;
    let callable_bodies = module
        .functions
        .iter()
        .map(|function| function.callable_body.id())
        .collect::<BTreeSet<_>>();
    let input = scoop_lir::SingleConeStrongLirOutput::try_new(module, Vec::new()).unwrap();

    let partition = StrongScoopLirObjectPartitionV1::from_input(&input).unwrap();

    assert_eq!(partition.objects().len(), callable_bodies.len() + 1);
    let non_callable = &partition.objects()[0];
    assert_eq!(non_callable.kind(), StrongScoopLirObjectKindV1::NonCallable);
    assert!(!non_callable.definition_plans().is_empty());
    let surface =
        scoop_lir::StrongObjectSymbolSurfaceV1::from_odr_free_foundation(input.foundation())
            .unwrap();
    assert!(non_callable.definition_plans().iter().all(|definition| {
        surface
            .plans()
            .iter()
            .find(|plan| plan.definition_plan() == *definition)
            .is_some_and(|plan| {
                plan.definition_role() != scoop_lir::StrongDefinitionRole::CallableBody
            })
    }));

    let actual_bodies = partition.objects()[1..]
        .iter()
        .map(|object| {
            assert_eq!(object.definition_plans().len(), 1);
            let StrongScoopLirObjectKindV1::CallableBody(body) = object.kind() else {
                panic!("only the first object may be non-callable");
            };
            body
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_bodies, callable_bodies);
}

#[test]
fn partition_is_complete_non_overlapping_and_excludes_generated_bridge_units() {
    let mut module = values_module();
    module.output = scoop_lir::LirOutput::Library;
    let input = scoop_lir::SingleConeStrongLirOutput::try_new(module, Vec::new()).unwrap();
    let producer_units =
        scoop_lir::StrongProducerUnitPartitionV1::from_odr_free_foundation(input.foundation())
            .unwrap();

    let partition = StrongScoopLirObjectPartitionV1::from_input(&input).unwrap();

    let actual = partition
        .objects()
        .iter()
        .flat_map(StrongScoopLirObjectUnitSetV1::definition_plans)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        actual.iter().copied().collect::<BTreeSet<_>>().len(),
        actual.len(),
        "one definition plan must not occur in multiple objects"
    );
    assert_eq!(
        actual.iter().copied().collect::<BTreeSet<_>>(),
        producer_units
            .scoop_lir_definition_plans()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
    );
    let generated_bridge_definitions = producer_units
        .generated_bridge_units()
        .iter()
        .flat_map(scoop_lir::GeneratedBridgeProducerUnitV1::definition_plans)
        .copied()
        .collect::<BTreeSet<_>>();
    assert!(
        actual
            .iter()
            .all(|plan| !generated_bridge_definitions.contains(plan))
    );
}
