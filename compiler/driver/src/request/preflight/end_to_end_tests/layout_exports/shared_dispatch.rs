use super::*;
use scoop_lir_lower::LayoutAbiExportInputV1;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let dispatch = expected.dispatch();
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
}
