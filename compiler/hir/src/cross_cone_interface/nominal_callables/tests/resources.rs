use super::*;
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

#[test]
fn nominal_scope_and_signature_projection_share_the_callers_budget() {
    let table = [nominal(crate::SourceNominalId::Concrete(foreign_type()))];
    let path = WirePath::root();
    for limits in [
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        let error = NominalExactLeafClassifierV1::try_from_nominal_interfaces_metered(
            &table,
            &mut BudgetMeter::new(limits),
            &path,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            super::super::NominalExactLeafClassifierBuildError::Resource(_)
        ));
    }
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    let classifier = NominalExactLeafClassifierV1::try_from_nominal_interfaces_metered(
        &table, &mut meter, &path,
    )
    .unwrap();
    let function = callable(
        SignatureTypeKey::Nominal(foreign_type()),
        Effect::Ordinary,
        CallableImplementationV1::Scoop,
        GcEffect::Managed,
    );
    assert!(
        classifier
            .classify_callable_metered(&function, &mut meter, &path)
            .unwrap()
            .is_some()
    );
    let mut exhausted = BudgetMeter::new(DecodeLimits {
        validation_work_units: 0,
        ..DecodeLimits::default()
    });
    assert!(matches!(
        classifier.classify_callable_metered(&function, &mut exhausted, &path),
        Err(super::super::NominalCallableClassificationError::Resource(
            _
        ))
    ));
}
