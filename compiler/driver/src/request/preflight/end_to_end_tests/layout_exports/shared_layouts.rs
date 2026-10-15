use super::*;
use scoop_lir_lower::LayoutAbiExportInputV1;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let layouts = expected.layouts();
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
}
