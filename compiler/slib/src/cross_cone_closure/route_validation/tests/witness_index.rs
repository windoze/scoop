use super::*;
use scoop_hir::{
    CanonicalDependencyBindingWitnessesV1, CanonicalExternalHirReferenceRolesV1,
    DependencyBindingWitnessV1, ExternalHirReferenceRoleV1, ExternalHirReferenceV1,
    PublicExportBindingClosureAuthority,
};
use scoop_wire::WirePath;

fn witness_interface(fixture: &RouteFixture) -> CrossConeHirInterfaceSectionV1 {
    let ExportBindingSourceV1::Reexport { routes } =
        fixture.current_interface.public_bindings().records()[0].source()
    else {
        panic!("expected re-export fixture");
    };
    let ExportBindingSourceV1::DeclaredCurrent { declaration } =
        fixture.terminal_interface.public_bindings().records()[0].source()
    else {
        panic!("expected terminal declaration");
    };
    let record = ExternalHirReferenceV1::try_new(
        fixture.terminal,
        (*declaration).into(),
        CanonicalExternalHirReferenceRolesV1::try_new(vec![
            ExternalHirReferenceRoleV1::ReexportTarget,
        ])
        .unwrap(),
        CanonicalDependencyBindingWitnessesV1::try_new(vec![DependencyBindingWitnessV1::new(
            routes.routes()[0].clone(),
        )])
        .unwrap(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let empty = interface(Vec::new());
    CrossConeHirInterfaceSectionV1::new(
        empty.public_bindings().clone(),
        empty.nominal_interfaces().clone(),
        empty.callable_interfaces().clone(),
        empty.property_interfaces().clone(),
        empty.type_aliases().clone(),
        empty.source_interfaces().clone(),
        empty.default_templates().clone(),
        empty.constants().clone(),
        empty.definition_sources().clone(),
        CanonicalExternalHirReferencesV1::try_new(vec![record]).unwrap(),
        empty.generic_callable_bodies().clone(),
        empty.generic_initializations().clone(),
    )
}

#[test]
fn binding_index_collects_external_witness_hops_before_role_closure_validation() {
    let fixture = route_fixture();
    let interface = witness_interface(&fixture);

    let authority = CanonicalCrossConeRouteAuthority::try_new(
        fixture.current,
        &fixture.identities,
        &interface,
        &[],
        &[],
        &WirePath::root(),
    )
    .unwrap();
    assert_eq!(authority.binding_keys.len(), 2);
    for hop in interface.external_references().records()[0]
        .witnesses()
        .witnesses()[0]
        .route()
        .hops()
    {
        assert!(authority.binding_key(hop.binding()).is_some());
    }
}
