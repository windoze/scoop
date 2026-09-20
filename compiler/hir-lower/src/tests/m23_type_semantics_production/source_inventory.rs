use super::*;
use scoop_identity::{PendingIdentityValidation, ValidatedIdentityGraph};
use scoop_wire::{WireDecode, WireEncode, decode_canonical, encode};

fn decoded<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

pub(super) fn identity_closure(output: &hir::OrdinaryHirOutput<'_>) -> ValidatedIdentityGraph {
    identity_closure_for_foundation(
        output,
        hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap(),
    )
}

pub(super) fn identity_closure_for_foundation(
    output: &hir::OrdinaryHirOutput<'_>,
    foundation: hir::CanonicalHirFoundation,
) -> ValidatedIdentityGraph {
    let core = trusted_core();
    let core_foundation: hir::DecodedHirFoundation = decoded(&core.foundation);
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    core_foundation.register_identities(&mut pending).unwrap();
    core_foundation.resolve_identities(&mut pending).unwrap();
    let core_graph = pending.finish().unwrap();

    let foundation: hir::DecodedHirFoundation = decoded(&foundation);
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_authority(output.output().export.cone)
        .unwrap();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    foundation.register_identities(&mut pending).unwrap();
    pending
        .register_external_graph_authorities(&core_graph)
        .unwrap();
    foundation.resolve_identities(&mut pending).unwrap();
    pending.finish().unwrap()
}

#[test]
fn ordinary_source_inventories_resolve_against_real_foundation_bytes() {
    let output = lower_public_nominals();
    let public = public_interface(&output);
    let production = produce_cross_cone_type_semantics(&output, &public).unwrap();
    let source = production.foundation();
    let mut identities = identity_closure(&output);
    let mut meter = BudgetMeter::new(DecodeLimits::default());

    let roots =
        hir::CanonicalSourceNominalIdsV1::try_new(source.source_roots().to_vec(), &mut meter)
            .unwrap();
    let roots = decoded::<hir::DecodedCanonicalSourceNominalIdsV1>(&roots)
        .resolve(&mut identities, &mut meter)
        .unwrap();
    assert_eq!(roots.values(), source.source_roots());

    let nominals = hir::CanonicalTypeSourceNominalsV1::try_new(
        source
            .source_nominals()
            .map(|(owner, access)| hir::TypeSourceNominalV1::new(owner, access.clone()))
            .collect(),
        &mut meter,
    )
    .unwrap();
    let restored = decoded::<hir::DecodedCanonicalTypeSourceNominalsV1>(&nominals)
        .resolve(&mut identities, &mut meter)
        .unwrap();
    assert_eq!(restored, nominals);
    assert_eq!(restored.records().len(), roots.values().len());

    let dependencies = hir::CanonicalTypeSectionDependencyFactsV1::try_new(
        source.dependency_facts().to_vec(),
        &mut meter,
    )
    .unwrap();
    let dependencies = decoded::<hir::DecodedCanonicalTypeSectionDependencyFactsV1>(&dependencies)
        .resolve(&mut identities, &mut meter)
        .unwrap();
    assert_eq!(dependencies.records(), source.dependency_facts());

    let edges = hir::CanonicalNominalInheritanceEdgesV1::try_new(
        source.local_inheritance_edges().to_vec(),
        &mut meter,
    )
    .unwrap();
    let edges = decoded::<hir::DecodedCanonicalNominalInheritanceEdgesV1>(&edges)
        .resolve(&mut identities, &mut meter)
        .unwrap();
    assert_eq!(edges.records(), source.local_inheritance_edges());
    let graph = hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
        edges.records().iter(),
        roots.values().iter().copied(),
        source,
        &mut meter,
    )
    .unwrap();
    for record in production.section().inheritance().records() {
        graph
            .validate_nominal_domains(record.owner(), record.domains(), &mut meter)
            .unwrap();
    }
}

#[test]
fn inheritance_inventory_is_independently_projected_before_candidate_and_survives_bytes() {
    let output = lower_public_nominals();
    let public = public_interface(&output);
    let production = produce_cross_cone_type_semantics(&output, &public).unwrap();
    let inventory = production.inheritance_inventory();
    let mut identities = identity_closure(&output);
    let restored = decoded::<hir::DecodedCanonicalSourceInheritanceInventoriesV1>(inventory)
        .resolve(
            &mut identities,
            &mut BudgetMeter::new(DecodeLimits::default()),
        )
        .unwrap();
    assert_eq!(&restored, inventory);
    assert_eq!(restored.owners().values().len(), 7);
    let mut constructors = 0;
    for entry in restored.records() {
        let candidate = production
            .section()
            .inheritance()
            .get(entry.owner())
            .unwrap();
        assert_eq!(
            entry.constructors().values(),
            candidate
                .constructors()
                .records()
                .iter()
                .map(|record| record.declaration())
                .collect::<Vec<_>>()
        );
        assert_eq!(entry.protected_members(), candidate.protected_members());
        assert_eq!(entry.slot_schemas(), candidate.slot_schemas());
        constructors += entry.constructors().values().len();
    }
    assert_eq!(constructors, 4);
    let interface_sources = hir::CanonicalInterfaceSourceDispatchesV1::from_ordinary_hir(
        &output,
        &mut BudgetMeter::new(DecodeLimits::default()),
    )
    .unwrap();
    assert_eq!(production.interface_sources(), &interface_sources);
    let slot_selections = hir::CanonicalInheritanceSourceSlotSelectionsV1::from_ordinary_hir(
        &output,
        &mut BudgetMeter::new(DecodeLimits::default()),
    )
    .unwrap();
    assert_eq!(production.slot_selections(), &slot_selections);
    let (candidate, foundation, inventory, source_interfaces, source_selections) =
        production.into_parts();
    assert_eq!(source_selections, slot_selections);
    assert_eq!(source_interfaces.records().len(), 1);
    assert_eq!(source_interfaces, interface_sources);
    assert_eq!(
        inventory.owners().values().len(),
        candidate.inheritance().records().len()
    );
    assert_eq!(
        foundation.local_inheritance_edges().len(),
        inventory.records().len()
    );
}

#[test]
fn producer_rejects_exhausted_source_inventory_budget() {
    let output = lower_public_nominals();
    let public = public_interface(&output);
    assert!(matches!(
        crate::produce_cross_cone_type_semantics(
            &output,
            &public,
            &mut BudgetMeter::new(DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(hir::CrossConeTypeSemanticsProductionError::SourceInventory(
            hir::SourceInventoryError::Resource(_)
        ))
    ));
}
