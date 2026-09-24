use super::*;
use scoop_lir_lower::LayoutAbiExportInputV1;
use scoop_slib::SharedLirShapeSupportValidationError as Error;

mod corruption;
mod resources;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let shapes = replay(
        input,
        expected.layouts(),
        expected.descriptors(),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(&shapes, expected.shape_support());
    let wire: lir::DecodedCanonicalParamFreeShapeSupportExportsV1 = decoded(&shapes);
    assert_eq!(
        wire.validate_against(&shapes, &mut meter()).unwrap(),
        shapes
    );
    assert_eq!(
        shapes.records().len(),
        input.bridge.shapes().records().len()
    );
    for (source, record) in input.bridge.shapes().records().iter().zip(shapes.records()) {
        assert_eq!(record.source_nominal(), source.source());
        assert_eq!(record.exact(), source.exact());
        assert_eq!(record.provider(), source.provider());
        assert_eq!(
            record.roles().coroutine_step().available().unwrap().exact(),
            source.coroutine_step()
        );
        assert_eq!(
            record.roles().coroutine_slot().available().unwrap().exact(),
            source.coroutine_slot()
        );
        match source.boxed() {
            mir::MirBoxedShapeSupportV1::Available(exact) => {
                assert_eq!(
                    record.roles().boxed_value().available().unwrap().exact(),
                    exact
                );
            }
            mir::MirBoxedShapeSupportV1::ReferenceNominalRequiresNoBox => {
                assert!(record.roles().boxed_value().available().is_none());
            }
        }
    }
}

pub(super) fn probe(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    corruption::check(input, expected);
    resources::check(input, expected);
}

fn replay(
    input: LayoutAbiExportInputV1<'_>,
    layouts: &lir::CanonicalExactLayoutExportsV1,
    descriptors: &lir::CanonicalExactDescriptorExportsV1,
    meter: &mut BudgetMeter,
) -> Result<lir::CanonicalParamFreeShapeSupportExportsV1, Error> {
    scoop_slib::replay_shared_mir_shape_support(
        input.bridge.shapes(),
        layouts,
        descriptors,
        input.identities,
        input.lir.foundation(),
        meter,
    )
}
