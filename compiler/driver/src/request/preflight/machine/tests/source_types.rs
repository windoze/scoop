use super::*;

#[test]
fn actual_core_type_surface_rejects_required_generic_inheritance() {
    let sources = sources::core_sources_with(&[]);
    let hir = super::super::super::TrustedCoreBootstrapHirOutput::lower(&sources).unwrap();
    let input = hir.machine_input();
    let error = scoop_hir::CrossConeTypeSemanticsProductionV1::from_dependency_hir(
        input.output,
        input.public,
        &mut scoop_wire::BudgetMeter::new(scoop_wire::DecodeLimits::default()),
    )
    .unwrap_err();
    let scoop_hir::CrossConeTypeSemanticsProductionError::GenericOdrRequired(exact) = error else {
        panic!("unexpected core type error: {error}");
    };
    let local = input.output.output().local.module();
    let ty = local
        .exact_type_identities
        .type_for_identity(exact)
        .unwrap();
    let scoop_hir::concrete::TypeKind::Interface(id) = local.types[ty].kind else {
        panic!(
            "expected a required generic interface: {:?}",
            local.types[ty].kind
        );
    };
    let interface = &local.interfaces[id];
    assert!(!interface.type_arguments.is_empty());
    assert!(
        matches!(interface.name.as_str(), "Iterable" | "Iterator"),
        "{}",
        interface.name
    );
    assert!(
        local
            .classes
            .iter()
            .any(|(_, class)| class.interfaces.contains(&ty))
    );
}
