use super::*;
use scoop_lir_lower::LayoutAbiExportInputV1;
use scoop_slib::{SharedLirDescriptorInputsV1, SharedLirDescriptorValidationError as Error};

mod corruption;
mod resources;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let descriptors = replay(
        input,
        expected.layouts(),
        expected.dispatch(),
        &[],
        &mut meter(),
    )
    .unwrap();
    assert_eq!(&descriptors, expected.descriptors());
    let wire: lir::DecodedCanonicalExactDescriptorExportsV1 = decoded(expected.descriptors());
    assert_eq!(
        wire.validate_against(&descriptors, &mut meter()).unwrap(),
        descriptors
    );
    assert!(matches!(
        replay(
            input,
            expected.layouts(),
            expected.dispatch(),
            &[&descriptors],
            &mut meter()
        ),
        Err(Error::DependencyProvider(_))
    ));
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
    dispatch: &lir::CanonicalExactDispatchExportsV1,
    dependencies: &[&lir::CanonicalExactDescriptorExportsV1],
    meter: &mut BudgetMeter,
) -> Result<lir::CanonicalExactDescriptorExportsV1, Error> {
    let diagnostics = scoop_identity::ExactTypeDiagnosticCatalog::try_new(
        input.identities,
        input.coordinates,
        meter,
    )?;
    scoop_slib::replay_shared_mir_descriptors(
        input.lir.module().meta.target_profile,
        input.bridge.types(),
        SharedLirDescriptorInputsV1 {
            layouts,
            dispatch,
            dependencies,
        },
        &diagnostics,
        input.lir.foundation(),
        meter,
    )
}
