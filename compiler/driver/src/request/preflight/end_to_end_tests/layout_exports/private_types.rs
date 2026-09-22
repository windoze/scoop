use super::*;

pub(super) fn check(
    input: scoop_lir_lower::LayoutAbiExportInputV1<'_>,
    exports: &lir::LayoutAbiExportConstituentsV1,
) {
    let (ty, exact) = identity(input, "LocalOnly");
    assert!(
        input
            .mir
            .materialization()
            .source_nominal_shape(&ty)
            .is_some()
    );
    assert!(input.bridge.types().get(exact).is_none());
    assert!(
        input
            .lir
            .module()
            .meta
            .type_descriptors
            .iter()
            .any(|(_, td)| { td.identity.exact_type() == exact })
    );
    assert!(
        input
            .registration
            .registration_production()
            .types()
            .registrations()
            .iter()
            .any(|registration| registration.exact_type() == exact)
    );
    assert!(exports.descriptors().get(exact).is_none());
    assert!(
        exports
            .layouts()
            .records()
            .iter()
            .all(|layout| layout.identity().exact() != exact)
    );
    assert!(
        exports
            .dispatch()
            .records()
            .iter()
            .all(|table| table.owner_exact() != exact)
    );
}

pub(super) fn check_support(
    input: scoop_lir_lower::LayoutAbiExportInputV1<'_>,
    dependencies: scoop_lir_lower::LayoutAbiExportDependenciesV1<'_>,
    exports: &lir::LayoutAbiExportConstituentsV1,
) {
    let (_, exact) = identity(input, "Hidden");
    assert!(input.bridge.types().get(exact).is_some());
    assert!(exports.descriptors().get(exact).is_some());
    assert!(
        exports
            .layouts()
            .records()
            .iter()
            .any(|layout| layout.identity().exact() == exact)
    );
    let incomplete = mir::MirTypeBridgeExportConstituentsV1::new(
        mir::CanonicalParamFreeMirTypeExportsV1::try_new(
            input
                .bridge
                .types()
                .records()
                .iter()
                .filter(|record| record.exact() != exact)
                .cloned()
                .collect(),
        )
        .unwrap(),
        input.bridge.callables().clone(),
        input.bridge.dispatch().clone(),
        input.bridge.objects().clone(),
        input.bridge.shapes().clone(),
        input.bridge.initialization_uses().clone(),
    );
    assert!(matches!(
        scoop_lir_lower::lower_layout_abi_exports(
            scoop_lir_lower::LayoutAbiExportInputV1 {
                bridge: &incomplete,
                ..input
            },
            dependencies,
            &mut meter(),
        ),
        Err(scoop_lir_lower::LayoutAbiExportLoweringError::Layout(
            scoop_lir_lower::ExactLayoutLoweringError::MissingDependency(_)
        ))
    ));
}

fn identity(
    input: scoop_lir_lower::LayoutAbiExportInputV1<'_>,
    name: &str,
) -> (mir::Type, scoop_identity::PersistentExactTypeId) {
    let (id, _) = input
        .mir
        .module()
        .structs
        .iter()
        .find(|(_, definition)| definition.name == name)
        .unwrap();
    let ty = mir::Type::Struct(id);
    let exact = input
        .mir
        .module()
        .meta
        .source_exact_types
        .get(&ty)
        .unwrap()
        .identity_record()
        .id();
    (ty, exact)
}
