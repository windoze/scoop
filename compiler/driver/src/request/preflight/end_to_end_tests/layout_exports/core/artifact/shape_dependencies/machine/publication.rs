//! Complete source and machine records share the same selected shape closure.

use super::*;

pub(super) fn check<'a, 'p>(
    source_input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    output: &lir::ConeLirOutput,
    selected: &lir::StrongProductionDependencySelectionV2<'a>,
    provider: Provider<'a, 'p>,
    source: PublicationInput<'_, '_>,
    coordinates: &[ConeCoordinate],
) -> (
    lir::ConeProductionSectionV2,
    lir::CrossConeLayoutAbiSectionV1<'a>,
) {
    let registration = output
        .build_production_section_v2(
            coordinates[1].clone(),
            &[provider.layout.provider()],
            lir::EntryProductionSourceV1::Library,
            &[],
        )
        .unwrap();
    let ordinary = scoop_lir_lower::lower_cross_cone_bridge_section(
        source_input.mir,
        source_input.ordinary,
        output,
    )
    .unwrap();
    let input = scoop_lir_lower::LayoutAbiExportInputV1 {
        mir: source_input.mir,
        lir: output,
        bridge: source.bridge.exports(),
        ordinary: &ordinary,
        registration: &registration,
        identities: source_input.identities,
        coordinates,
    };
    let dependencies = scoop_lir_lower::LayoutAbiExportDependenciesV1 {
        layouts: &[provider.layout.layouts()],
        callables: &[provider.layout.callables()],
        direct_callables: &[provider.layout.direct_callables()],
    };
    let exports = scoop_lir_lower::lower_layout_abi_exports(input, dependencies).unwrap();
    let imports = selected.physical_imports().records().to_vec();
    let committed = source.source;
    let roots =
        scoop_lir_lower::lower_layout_abi_dependencies(input, dependencies, &exports, committed)
            .unwrap();
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
        .materialized_shape_dependencies(source_input.mir.module().cone, source_input.identities)
        .unwrap();
    assert_eq!(actual_shapes, expected_shapes);
    assert!(!actual_shapes.is_empty());
    let section =
        lir::CrossConeLayoutAbiSectionV1::try_new(exports, &[provider.layout], imports, &roots)
            .unwrap();
    let production = registration;
    assert_eq!(
        production.type_registrations().registrations().len(),
        output.module().meta.type_descriptors.len()
    );
    assert_eq!(
        production.callable_registrations().registrations().len(),
        output.module().functions.len()
    );
    (production, section)
}
