use super::*;
use scoop_lir_lower::{LayoutAbiExportDependenciesV1, LayoutAbiExportInputV1};
use scoop_slib::SharedLirLayoutValidationError as Error;

mod corruption;
mod dump;
mod resources;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    dependencies: LayoutAbiExportDependenciesV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let layouts = replay(
        input,
        input.bridge.types(),
        dependencies.layouts,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(&layouts, expected.layouts());
    for source in input.bridge.types().records() {
        let mir::MirTypeRepresentationV1::Object { backing } = source.representation() else {
            continue;
        };
        assert!(
            layouts
                .records()
                .iter()
                .all(|layout| layout.identity().exact() != *backing)
        );
        assert!(
            layouts
                .find_exact_role(
                    source.exact(),
                    scoop_identity::RepresentationRole::ManagedObject
                )
                .is_some()
        );
    }
    let wire: lir::DecodedCanonicalExactLayoutExportsV1 = decoded(expected.layouts());
    assert_eq!(
        wire.validate_against(&layouts, &mut meter()).unwrap(),
        layouts
    );
    if !dependencies.layouts.is_empty() {
        let without_dependencies = replay(input, input.bridge.types(), &[], &mut meter());
        if has_foreign_layout_dependency(input.bridge.types()) {
            assert!(matches!(
                without_dependencies,
                Err(Error::MissingDependency(_))
            ));
        } else {
            assert_eq!(without_dependencies.unwrap(), layouts);
        }
        let mut repeated = dependencies.layouts.to_vec();
        repeated.push(dependencies.layouts[0]);
        assert!(matches!(
            replay(input, input.bridge.types(), &repeated, &mut meter()),
            Err(Error::DependencyProvider(_))
        ));
    }
    let local_as_dependency = [expected.layouts()];
    assert!(matches!(
        replay(input, input.bridge.types(), &local_as_dependency, &mut meter()),
        Err(Error::DependencyProvider(provider)) if provider == expected.provider()
    ));
}

fn has_foreign_layout_dependency(types: &mir::CanonicalParamFreeMirTypeExportsV1) -> bool {
    types.records().iter().any(|record| {
        let shape = record.representation();
        let base = match record.base_and_interfaces().base {
            mir::MirBaseClassV1::None => None,
            mir::MirBaseClassV1::Base(exact) => Some(exact),
        };
        shape
            .fields()
            .iter()
            .map(|field| field.value)
            .chain(
                shape
                    .variants()
                    .iter()
                    .flat_map(|variant| variant.fields.iter().map(|field| field.value)),
            )
            .chain(base)
            .any(|exact| types.get(exact).is_none())
    })
}

pub(super) fn probe(
    name: &str,
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    corruption::check(input, expected.layouts());
    resources::check(input);
    dump::check(name, input, expected.layouts());
}

fn replay(
    input: LayoutAbiExportInputV1<'_>,
    types: &mir::CanonicalParamFreeMirTypeExportsV1,
    dependencies: &[&lir::CanonicalExactLayoutExportsV1],
    meter: &mut BudgetMeter,
) -> Result<lir::CanonicalExactLayoutExportsV1, Error> {
    scoop_slib::replay_shared_mir_layouts(
        input.lir.module().meta.target_profile,
        types,
        input.lir.foundation(),
        input.identities,
        dependencies,
        meter,
    )
}
