use super::*;
use scoop_identity::{PendingIdentityValidation, ValidatedIdentityGraph};
use scoop_wire::{WireDecode, WireEncode, decode_canonical, encode};

fn decoded<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

pub(super) fn identity_closure(output: &hir::DependencyHirOutput) -> ValidatedIdentityGraph {
    let mut foundation = hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap();
    let interface = public_interface(output);
    foundation
        .complete_cross_cone_interface_source_points(output.output().export.module(), &interface)
        .unwrap();
    identity_closure_for_foundation(output, foundation)
}

pub(super) fn identity_closure_for_foundation(
    output: &hir::DependencyHirOutput,
    foundation: hir::CanonicalHirFoundation,
) -> ValidatedIdentityGraph {
    let core_graph = core_identity_closure();
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

pub(super) fn core_identity_closure() -> ValidatedIdentityGraph {
    let core = trusted_core();
    let foundation: hir::DecodedHirFoundation = decoded(&core.foundation);
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    foundation.register_identities(&mut pending).unwrap();
    foundation.resolve_identities(&mut pending).unwrap();
    pending.finish().unwrap()
}

#[test]
fn inheritance_inventory_is_independently_projected_before_candidate_and_survives_bytes() {
    let output = lower_public_nominals();
    let public = public_interface(&output);
    let production = produce_cross_cone_type_semantics(&output, &public).unwrap();
    let inventory = production.inheritance_inventory();
    let mut identities = identity_closure(&output);
    let restored = decoded::<hir::DecodedCanonicalSourceInheritanceInventoriesV1>(inventory)
        .resolve(&mut identities)
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
    let interface_sources =
        hir::CanonicalInterfaceSourceDispatchesV1::from_dependency_hir(&output).unwrap();
    assert_eq!(production.interface_sources(), &interface_sources);
    let slot_selections =
        hir::CanonicalInheritanceSourceSlotSelectionsV1::from_dependency_hir(&output).unwrap();
    assert_eq!(production.slot_selections(), &slot_selections);
    let source_callables =
        hir::CanonicalInheritanceSourceCallablesV1::from_dependency_hir(&output).unwrap();
    assert_eq!(production.source_callables(), &source_callables);
    let source_constructors =
        hir::CanonicalInheritanceSourceConstructorsV1::from_dependency_hir(&output).unwrap();
    assert_eq!(production.source_constructors(), &source_constructors);
    let source_protected_callables =
        hir::CanonicalInheritanceSourceProtectedCallablesV1::from_dependency_hir(&output).unwrap();
    assert_eq!(
        production.source_protected_callables(),
        &source_protected_callables
    );
    let (
        candidate,
        foundation,
        inventory,
        source_interfaces,
        source_selections,
        callables,
        constructors,
        protected_callables,
        properties,
        parameters,
        nominal_contracts,
    ) = production.into_parts();
    let required_nominals =
        hir::CanonicalSourceNominalIdsV1::try_new(foundation.source_roots().to_vec()).unwrap();
    assert_eq!(
        nominal_contracts,
        hir::CanonicalNominalSourceContractsV1::from_export_hir(
            &output.output().export,
            &required_nominals
        )
        .unwrap()
    );
    assert_eq!(
        parameters,
        hir::CanonicalInheritanceSourceParameterProtocolsV1::from_dependency_hir(&output).unwrap()
    );
    assert_eq!(
        properties,
        hir::CanonicalInheritanceSourcePropertiesV1::from_dependency_hir(&output).unwrap()
    );
    assert_eq!(protected_callables, source_protected_callables);
    assert_eq!(constructors, source_constructors);
    assert_eq!(callables, source_callables);
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
