use super::*;
use scoop_lir_lower::{LayoutAbiExportDependenciesV1, LayoutAbiExportInputV1};
use scoop_slib::{SharedLirDispatchAbiInputsV1, SharedLirDispatchValidationError as Error};

mod corruption;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    dependencies: LayoutAbiExportDependenciesV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let inputs = SharedLirDispatchAbiInputsV1 {
        local_layouts: expected.layouts(),
        local_callables: expected.callables(),
        dependency_layouts: dependencies.layouts,
        dependency_callables: dependencies.callables,
    };
    let dispatch = replay(input, inputs).unwrap();
    assert_eq!(&dispatch, expected.dispatch());
    let wire: lir::DecodedCanonicalExactDispatchExportsV1 = decoded(expected.dispatch());
    assert_eq!(wire.validate_against(&dispatch).unwrap(), dispatch);
    for ty in input.bridge.types().records() {
        let tables: Vec<_> = dispatch
            .records()
            .iter()
            .filter(|table| table.owner_exact() == ty.exact())
            .collect();
        if matches!(
            ty.representation(),
            mir::MirTypeRepresentationV1::ObjectBacking { .. }
        ) {
            assert!(tables.is_empty());
        } else {
            assert_eq!(
                tables
                    .iter()
                    .filter(|table| table.role() == lir::ExactDispatchRoleV1::Vtable)
                    .count(),
                1
            );
        }
        if matches!(
            ty.representation(),
            mir::MirTypeRepresentationV1::Struct { .. }
                | mir::MirTypeRepresentationV1::Enum { .. }
                | mir::MirTypeRepresentationV1::Interface
        ) {
            assert_eq!(tables.len(), 1);
            assert!(tables[0].entries().is_empty());
        }
    }
    assert!(matches!(
        replay(
            input,
            SharedLirDispatchAbiInputsV1 {
                dependency_layouts: &[expected.layouts()],
                ..inputs
            }
        ),
        Err(Error::DependencyProvider(_))
    ));
    assert!(matches!(
        replay(
            input,
            SharedLirDispatchAbiInputsV1 {
                dependency_callables: &[expected.callables()],
                ..inputs
            }
        ),
        Err(Error::DependencyProvider(_))
    ));
    if let Some(dependency) = dependencies.layouts.first() {
        assert!(matches!(
            replay(
                input,
                SharedLirDispatchAbiInputsV1 {
                    local_layouts: dependency,
                    ..inputs
                }
            ),
            Err(Error::LocalProvider)
        ));
        assert!(matches!(
            replay(
                input,
                SharedLirDispatchAbiInputsV1 {
                    dependency_layouts: &[dependency, dependency],
                    ..inputs
                }
            ),
            Err(Error::DependencyProvider(_))
        ));
    }
}

pub(super) fn probe(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let inputs = SharedLirDispatchAbiInputsV1 {
        local_layouts: expected.layouts(),
        local_callables: expected.callables(),
        dependency_layouts: &[],
        dependency_callables: &[],
    };
    corruption::check(input, expected, inputs);
}

fn replay(
    input: LayoutAbiExportInputV1<'_>,
    abis: SharedLirDispatchAbiInputsV1<'_>,
) -> Result<lir::CanonicalExactDispatchExportsV1, Error> {
    scoop_slib::replay_shared_mir_dispatch(
        input.lir.module().meta.target_profile,
        input.bridge.types(),
        input.bridge.dispatch(),
        abis,
        input.lir.foundation(),
    )
}
