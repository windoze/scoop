use super::*;
use scoop_lir_lower::LayoutAbiExportInputV1;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let shapes = expected.shape_support();
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
