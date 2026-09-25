use super::*;
use scoop_identity::{ExactTypeKey, PersistentExactTypeId};
use scoop_slib::CrossConeTypeDescriptorProjectionError as Error;

pub(super) fn check(
    closure: &scoop_slib::ValidatedCrossConeSemanticClosure,
    core: &Compile<'_, '_>,
    ordinary: &Compile<'_, '_>,
) {
    let string = core
        .production()
        .hir_core()
        .compiler_protocol_definitions()
        .unwrap()
        .string_capability();
    let empty = ordinary
        .production()
        .hir_interface()
        .nominal_interfaces()
        .records()
        .iter()
        .find(|record| record.kind() == scoop_hir::PublicNominalKindV1::Struct)
        .unwrap();
    let scoop_hir::SourceNominalId::Concrete(nominal) = empty.declaration() else {
        panic!("fixture has a concrete source struct")
    };
    let empty_exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap();
    for (provider, source, exact) in [
        (core.identity(), string.source_type(), string.exact_type()),
        (ordinary.identity(), nominal, empty_exact),
    ] {
        let descriptor = closure
            .project_source_type_descriptor(provider, source)
            .unwrap();
        assert_eq!(descriptor.provider(), provider);
        assert_eq!(descriptor.target(), exact);
        assert_eq!(
            closure.project_type_descriptor(provider, exact).unwrap(),
            descriptor
        );
    }
    assert!(matches!(
        closure.project_source_type_descriptor(core.identity(), nominal),
        Err(Error::MissingSourceNominal { .. })
    ));
    assert!(matches!(
        closure.project_source_type_descriptor(ordinary.identity(), string.source_type()),
        Err(Error::MissingSourceNominal { .. })
    ));
    for (provider, exact) in [
        (core.identity(), empty_exact),
        (ordinary.identity(), string.exact_type()),
    ] {
        assert!(matches!(
            closure.project_type_descriptor(provider, exact),
            Err(Error::MissingShapeSupport { .. })
        ));
    }
    assert!(matches!(
        closure.project_type_descriptor(closure.current(), string.exact_type()),
        Err(Error::MissingProvider(_))
    ));
}
