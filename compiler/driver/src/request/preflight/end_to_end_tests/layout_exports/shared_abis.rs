use super::*;
use scoop_lir_lower::{LayoutAbiExportDependenciesV1, LayoutAbiExportInputV1};
use scoop_slib::SharedLirCallableAbiValidationError as Error;

mod corruption;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    dependencies: LayoutAbiExportDependenciesV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let layouts = scoop_slib::replay_shared_mir_layouts(
        input.lir.module().meta.target_profile,
        input.bridge.types(),
        input.lir.foundation(),
        input.identities,
        dependencies.layouts,
    )
    .unwrap();
    let abis = replay(input, &layouts, dependencies.layouts, expected.callables()).unwrap();
    assert_eq!(&abis, expected.callables());
    let wire: lir::DecodedCanonicalExactCallableAbiExportsV1 = decoded(expected.callables());
    assert_eq!(wire.validate_against(&abis).unwrap(), abis);
    if !dependencies.layouts.is_empty() {
        let missing = replay(input, &layouts, &[], &abis);
        let has_foreign = input.bridge.callables().entries().iter().any(|binding| {
            let signature = binding.lowered_signature().exact();
            signature
                .receiver()
                .into_option()
                .into_iter()
                .chain(signature.parameters().iter().copied())
                .chain([signature.result()])
                .any(|exact| input.bridge.types().get(exact).is_none())
        });
        if has_foreign {
            assert!(matches!(
                missing,
                Err(Error::Abi(
                    lir::ExactCallableAbiError::MissingValueLayout { .. }
                ))
            ));
        } else {
            assert_eq!(missing.unwrap(), abis);
        }
        let mut duplicate = dependencies.layouts.to_vec();
        duplicate.push(dependencies.layouts[0]);
        assert!(matches!(
            replay(input, &layouts, &duplicate, &abis),
            Err(Error::DependencyProvider(_))
        ));
        assert!(matches!(
            replay(input, dependencies.layouts[0], &[], &abis),
            Err(Error::LocalProvider)
        ));
    }
    assert!(matches!(
        replay(input, &layouts, &[&layouts], &abis),
        Err(Error::DependencyProvider(_))
    ));
}

pub(super) fn probe(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    corruption::check(input, expected);
}

fn replay(
    input: LayoutAbiExportInputV1<'_>,
    local: &lir::CanonicalExactLayoutExportsV1,
    dependencies: &[&lir::CanonicalExactLayoutExportsV1],
    actual: &lir::CanonicalExactCallableAbiExportsV1,
) -> Result<lir::CanonicalExactCallableAbiExportsV1, Error> {
    scoop_slib::validate_shared_mir_callable_abis(
        input.lir.module().meta.target_profile,
        input.bridge.callables(),
        local,
        dependencies,
        input.lir.foundation(),
        input.identities,
        actual,
    )?;
    Ok(actual.clone())
}
