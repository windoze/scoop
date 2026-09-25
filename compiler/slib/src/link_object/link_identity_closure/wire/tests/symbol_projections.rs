use super::*;
use scoop_wire::{ResourceKind, WireErrorKind, WirePath};

#[test]
fn borrowed_symbol_projection_replays_actual_owners_and_preserves_wire() {
    let expected = closure();
    let bytes = encode(&expected).unwrap();
    let mut wire =
        decode_canonical::<DecodedLinkIdentityClosureSectionV1>(&bytes, DecodeLimits::default())
            .unwrap();
    wire.replay_symbol_projections(
        &expected.defined_symbols,
        &expected.undefined_symbols,
        &mut BudgetMeter::new(DecodeLimits::default()),
    )
    .unwrap();
    assert_eq!(encode(&wire).unwrap(), bytes);
    wire.defined_symbols = decode_canonical(&[0x80], DecodeLimits::default()).unwrap();
    assert!(matches!(
        wire.replay_symbol_projections(
            &expected.defined_symbols,
            &expected.undefined_symbols,
            &mut BudgetMeter::new(DecodeLimits::default())
        ),
        Err(LinkSymbolProjectionValidationError::DefinedSymbols(
            DefinedLinkSymbolOwnerValidationError::ProjectionMismatch
        ))
    ));
}

#[test]
fn borrowed_symbol_projection_budgets_are_inclusive_and_cumulative() {
    let expected = closure();
    let bytes = encode(&expected).unwrap();
    let wire =
        decode_canonical::<DecodedLinkIdentityClosureSectionV1>(&bytes, DecodeLimits::default())
            .unwrap();
    let replay = |meter: &mut BudgetMeter| {
        wire.replay_symbol_projections(
            &expected.defined_symbols,
            &expected.undefined_symbols,
            meter,
        )
    };
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    replay(&mut meter).unwrap();
    let usage = meter.usage();
    let mut exact = BudgetMeter::new(DecodeLimits {
        validation_work_units: usage.validation_work_units,
        owned_bytes: usage.owned_bytes,
        ..DecodeLimits::default()
    });
    replay(&mut exact).unwrap();
    for (limits, resource, prefix) in [
        (
            DecodeLimits {
                validation_work_units: usage.validation_work_units - 1,
                ..DecodeLimits::default()
            },
            ResourceKind::ValidationWorkUnits,
            false,
        ),
        (
            DecodeLimits {
                validation_work_units: usage.validation_work_units,
                ..DecodeLimits::default()
            },
            ResourceKind::ValidationWorkUnits,
            true,
        ),
        (
            DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::OwnedBytes,
            false,
        ),
        (
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::LogicalHeapBytes,
            false,
        ),
    ] {
        let mut meter = BudgetMeter::new(limits);
        if prefix {
            meter.charge_work(1, &WirePath::root()).unwrap();
        }
        assert!(
            matches!(replay(&mut meter), Err(LinkSymbolProjectionValidationError::Resource(error))
            if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: actual, .. } if *actual == resource))
        );
        assert_eq!(encode(&wire).unwrap(), bytes);
    }
}
