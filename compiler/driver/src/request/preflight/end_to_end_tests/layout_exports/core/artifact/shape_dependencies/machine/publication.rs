//! Complete source and machine records share the same selected shape closure.

use super::*;
use lir::LayoutAbiSectionSourceAuthorityV1;

pub(super) fn check(
    source_input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    output: &lir::SingleConeStrongLirOutput,
    selected: &lir::StrongProductionDependencySelectionV2<'_>,
    provider: Provider<'_, '_>,
    source: PublicationInput<'_>,
    coordinates: &[ConeCoordinate],
) -> (lir::ValidatedStrongProductionSectionV2, String) {
    let registration = output
        .build_production_section_v2(
            coordinates[1].clone(),
            &[provider.layout.provider()],
            lir::EntryProductionSourceV1::Library,
            selected,
            &[],
            &mut meter(),
        )
        .unwrap();
    let input = scoop_lir_lower::LayoutAbiExportInputV1 {
        mir: source_input.mir,
        lir: output,
        bridge: source.bridge,
        registration: &registration,
        identities: source_input.identities,
        coordinates,
    };
    let dependencies = scoop_lir_lower::LayoutAbiExportDependenciesV1 {
        layouts: &[provider.layout.layouts()],
        callables: &[provider.layout.callables()],
    };
    let exports =
        scoop_lir_lower::lower_layout_abi_exports(input, dependencies, &mut meter()).unwrap();
    let projection = scoop_lir_lower::LayoutAbiSourceProjectionV1::from_input(
        input,
        dependencies,
        source.source,
        &mut meter(),
    )
    .unwrap();
    let roots = projection.committed_semantic_roots().unwrap();
    let actual_shapes = roots
        .iter()
        .filter_map(|root| match root.target() {
            lir::LayoutAbiSemanticTargetV1::ShapeSupport(source) => Some((root.provider(), source)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let expected_shapes = source_input
        .public
        .external_references()
        .materialized_shape_dependencies(
            source_input.mir.module().cone,
            source_input.identities,
            &mut meter(),
        )
        .unwrap();
    assert_eq!(actual_shapes, expected_shapes);
    assert!(!actual_shapes.is_empty());
    assert!(matches!(
        projection.validate_physical_imports(&[], &mut meter()),
        Err(scoop_lir_lower::LayoutAbiSourceProjectionError::PhysicalInventory)
    ));
    let imports = selected
        .physical_imports()
        .records()
        .iter()
        .filter(|import| {
            import.subject() != lir::ExternalStrongShapeSubjectV1::TypeDescriptor(provider.string)
        })
        .cloned()
        .collect();
    let section = lir::CrossConeLayoutAbiSectionV1::try_new(
        exports,
        &[provider.layout],
        imports,
        &projection,
        &mut meter(),
    )
    .unwrap();
    let relations = section.selected().semantic_relations().collect::<Vec<_>>();
    let decoded = super::super::super::lir_dependencies::corruption::wire::resolve(
        &section,
        &relations,
        source_input.identities,
    )
    .unwrap();
    let foundation = hir::OdrFreeHirFoundation::try_new(
        hir::CanonicalHirFoundation::from_type_semantics_output(source_input.hir).unwrap(),
    )
    .unwrap();
    scoop_slib::replay_shared_lir_dependency_graph(
        hir::SharedTypeMetadataV1 {
            provider: source_input.mir.module().cone,
            identities: source_input.identities,
            foundation: &foundation,
            public: source_input.public,
        },
        &decoded,
        &[provider.layout.exports()],
        &mut meter(),
    )
    .unwrap();
    let production = registration
        .validate_layout_abi(&section, &mut meter())
        .unwrap();
    assert!(production.type_registrations().registrations().is_empty());
    assert_eq!(
        production.callable_registrations().registrations().len(),
        output.module().functions.len()
    );
    let dump = relations
        .iter()
        .map(|relation| format!("{} {:?}\n", relation.provider(), relation.target()))
        .collect();
    (production, dump)
}
