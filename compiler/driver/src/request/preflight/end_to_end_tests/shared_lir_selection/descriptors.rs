use super::*;
use scoop_identity::{ExactTypeKey, PersistentExactTypeId};
use scoop_lir::*;

pub(super) fn check(
    closure: &scoop_slib::ValidatedCrossConeSemanticClosure,
    core: &Compile<'_, '_>,
    ordinary: &Compile<'_, '_>,
) {
    let string = core
        .production()
        .hir_core()
        .compiler_protocol_definitions()
        .unwrap()
        .string_capability();
    let empty = ordinary
        .production()
        .hir_interface()
        .nominal_interfaces()
        .records()
        .iter()
        .find(|record| record.kind() == scoop_hir::PublicNominalKindV1::Struct)
        .unwrap();
    let scoop_hir::SourceNominalId::Concrete(nominal) = empty.declaration() else {
        panic!("fixture has a concrete source struct")
    };
    let empty_exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap();
    let dependencies = closure.layout_dependencies().collect::<Vec<_>>();
    let roots = [
        LayoutAbiDependencyV1::new(
            core.identity(),
            LayoutAbiSemanticTargetV1::Descriptor(string.exact_type()),
        ),
        LayoutAbiDependencyV1::new(
            ordinary.identity(),
            LayoutAbiSemanticTargetV1::Descriptor(empty_exact),
        ),
    ];
    let imports = roots
        .iter()
        .map(|root| {
            let dependency = dependencies
                .iter()
                .find(|dependency| dependency.identity() == root.provider())
                .unwrap();
            let exports = dependency.lir_exports();
            let provider = ShapeLinkProviderV1::try_new(ShapeLinkProviderPartsV1 {
                foundation: dependency.lir_foundation(),
                production: dependency.lir_strong_production(),
                ordinary: dependency.lir_cross_cone_bridge(),
                layouts: exports.layouts(),
                callables: exports.callables(),
                descriptors: exports.descriptors(),
                dispatch: exports.dispatch(),
            })
            .unwrap();
            let LayoutAbiSemanticTargetV1::Descriptor(exact) = root.target() else {
                unreachable!()
            };
            ExternalShapeLinkImportV1::replay(
                &provider,
                ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
                closure.current(),
            )
            .unwrap()
        })
        .collect();
    let selected = StrongProductionDependencySelectionV2::try_new(
        closure.current(),
        dependencies[0].lir_exports().target_profile(),
        &dependencies
            .iter()
            .map(|dependency| dependency.lir_exports())
            .collect::<Vec<_>>(),
        imports,
        &roots,
    )
    .unwrap();
    for (provider, exact) in [
        (core.identity(), string.exact_type()),
        (ordinary.identity(), empty_exact),
    ] {
        let descriptor = selected
            .materialize_type_descriptor(provider, exact)
            .unwrap();
        assert_eq!(descriptor.provider(), provider);
        assert_eq!(descriptor.target(), exact);
    }
    for (provider, exact) in [
        (core.identity(), empty_exact),
        (ordinary.identity(), string.exact_type()),
    ] {
        assert!(matches!(
            selected.materialize_type_descriptor(provider, exact),
            Err(LayoutExternalMaterializationError::MissingDescriptor { .. })
        ));
    }
    assert!(
        selected
            .materialize_type_descriptor(closure.current(), string.exact_type())
            .is_err()
    );
}
