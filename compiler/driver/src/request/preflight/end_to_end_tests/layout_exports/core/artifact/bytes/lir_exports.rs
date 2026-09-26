use super::*;

pub(super) fn check(
    name: &str,
    artifact: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
    callables: scoop_slib::MirSourceCallablesValidatedCrossConeLayoutClosure<'_>,
    units: &[mir::MirTypeBridgeInitializationUnitV1],
    mir: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    lir: &lir::CrossConeLayoutAbiSectionV1<'_>,
) {
    let layouts = callables
        .validate_lir_layouts()
        .unwrap_or_else(|error| panic!("{name} shared LIR layout replay: {error}"));
    assert_eq!(layouts.current(), ConeIdentity::CORE);
    assert_eq!(layouts.target_selection(), artifact.target_selection());
    assert!(layouts.direct_providers().is_empty());
    assert_eq!(layouts.dependency_count(ConeIdentity::CORE), Some(0));
    assert_eq!(layouts.dependency_first().count(), 1);
    let current = layouts.artifact(ConeIdentity::CORE).unwrap();
    assert_eq!(current.layouts(), lir.layouts());
    assert_eq!(current.types(), mir.types());
    assert_eq!(current.callables(), mir.callables());
    assert_eq!(current.initialization_units(), units);
    assert_eq!(
        encode(current.lir_layout_abi_wire()).unwrap(),
        encode(lir).unwrap()
    );
    let abis = layouts
        .validate_lir_callable_abis()
        .unwrap_or_else(|error| panic!("{name} shared LIR callable ABI replay: {error}"));
    assert_eq!(abis.current(), ConeIdentity::CORE);
    assert_eq!(abis.target_selection(), artifact.target_selection());
    assert!(abis.direct_providers().is_empty());
    assert_eq!(abis.dependency_count(ConeIdentity::CORE), Some(0));
    assert_eq!(abis.dependency_first().count(), 1);
    let current = abis.artifact(ConeIdentity::CORE).unwrap();
    assert_eq!(current.layouts(), lir.layouts());
    assert_eq!(current.callable_abis(), lir.callables());
    assert_eq!(current.types(), mir.types());
    assert_eq!(current.callables(), mir.callables());
    assert_eq!(current.initialization_units(), units);
    assert_eq!(
        encode(current.lir_layout_abi_wire()).unwrap(),
        encode(lir).unwrap()
    );
    let dispatch = abis
        .validate_lir_dispatch()
        .unwrap_or_else(|error| panic!("{name} shared LIR dispatch replay: {error}"));
    assert_eq!(dispatch.current(), ConeIdentity::CORE);
    assert_eq!(dispatch.target_selection(), artifact.target_selection());
    assert!(dispatch.direct_providers().is_empty());
    assert_eq!(dispatch.dependency_count(ConeIdentity::CORE), Some(0));
    assert_eq!(dispatch.dependency_first().count(), 1);
    let current = dispatch.artifact(ConeIdentity::CORE).unwrap();
    assert_eq!(current.layouts(), lir.layouts());
    assert_eq!(current.callable_abis(), lir.callables());
    assert_eq!(current.lir_dispatch(), lir.dispatch());
    assert_eq!(current.types(), mir.types());
    assert_eq!(current.dispatch(), mir.dispatch());
    assert_eq!(current.initialization_units(), units);
    assert_eq!(
        encode(current.lir_layout_abi_wire()).unwrap(),
        encode(lir).unwrap()
    );
    let descriptors = dispatch
        .validate_lir_descriptors()
        .unwrap_or_else(|error| panic!("{name} shared LIR descriptor replay: {error}"));
    assert_eq!(descriptors.current(), ConeIdentity::CORE);
    assert_eq!(descriptors.target_selection(), artifact.target_selection());
    assert_eq!(descriptors.dependency_first().count(), 1);
    let current = descriptors.artifact(ConeIdentity::CORE).unwrap();
    assert_eq!(current.layouts(), lir.layouts());
    assert_eq!(current.callable_abis(), lir.callables());
    assert_eq!(current.lir_dispatch(), lir.dispatch());
    assert_eq!(current.descriptors(), lir.descriptors());
    assert_eq!(current.initialization_units(), units);
    assert_eq!(
        encode(current.lir_layout_abi_wire()).unwrap(),
        encode(lir).unwrap()
    );
    let complete = descriptors
        .validate_lir_shape_support()
        .unwrap_or_else(|error| panic!("{name} shared LIR shape-support replay: {error}"));
    assert_eq!(complete.current(), ConeIdentity::CORE);
    assert_eq!(complete.target_selection(), artifact.target_selection());
    assert_eq!(complete.dependency_first().count(), 1);
    let current = complete.artifact(ConeIdentity::CORE).unwrap();
    assert_eq!(current.lir_exports().layouts(), lir.layouts());
    assert_eq!(current.lir_exports().callables(), lir.callables());
    assert_eq!(current.lir_exports().dispatch(), lir.dispatch());
    assert_eq!(current.lir_exports().descriptors(), lir.descriptors());
    assert_eq!(current.lir_exports().shape_support(), lir.shape_support());
    assert_eq!(current.initialization_units(), units);
    assert_eq!(
        encode(current.lir_layout_abi_wire()).unwrap(),
        encode(lir).unwrap()
    );
    let expected_ordinary = encode(current.lir_cross_cone_bridge_wire()).unwrap();
    let complete = complete
        .validate_ordinary_lir_bridges()
        .unwrap_or_else(|error| panic!("{name} shared ordinary LIR bridge replay: {error}"));
    let current = complete.artifact(ConeIdentity::CORE).unwrap();
    assert_eq!(current.lir_exports().layouts(), lir.layouts());
    assert_eq!(current.lir_exports().shape_support(), lir.shape_support());
    assert_eq!(
        encode(current.lir_cross_cone_bridge()).unwrap(),
        expected_ordinary
    );
    super::lir_strong::check(name, complete, units, lir);
}
