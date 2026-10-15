use super::*;
use scoop_lir_lower::LayoutAbiExportInputV1;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let descriptors = expected.descriptors();
    for descriptor in descriptors.records() {
        assert!(
            descriptor
                .ancestry()
                .interfaces()
                .windows(2)
                .all(|pair| pair[0].exact_type() < pair[1].exact_type())
        );
        let physical = input
            .lir
            .module()
            .meta
            .type_descriptors
            .iter()
            .find(|(_, value)| value.identity.exact_type() == descriptor.exact())
            .unwrap()
            .1;
        assert!(
            physical
                .itables
                .windows(2)
                .all(|pair| pair[0].identity_record().key().interface()
                    < pair[1].identity_record().key().interface())
        );
    }
}
