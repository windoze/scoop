use super::*;
use scoop_lir::DecodedStrongProductionSectionV2;
use scoop_wire::{DecodeLimits, ResourceKind, WireErrorKind, decode_canonical, encode};

#[test]
fn original_strong_wire_recomputes_the_exact_producer_code() {
    with_empty_layout_code_fixture(|fixture| {
        let code = fixture.code.code();
        let bytes = encode(code.production().strong_production()).unwrap();
        let decoded =
            decode_canonical::<DecodedStrongProductionSectionV2>(&bytes, DecodeLimits::default())
                .unwrap();
        let input = replay_input(code, &decoded);
        assert_eq!(input.fingerprint().unwrap(), code.fingerprint());
        assert_eq!(encode(&decoded).unwrap(), bytes);
    });
}

#[test]
fn original_strong_code_hash_uses_the_same_budget_at_the_inclusive_boundary() {
    with_empty_layout_code_fixture(|fixture| {
        let code = fixture.code.code();
        let bytes = encode(code.production().strong_production()).unwrap();
        let decoded =
            decode_canonical::<DecodedStrongProductionSectionV2>(&bytes, DecodeLimits::default())
                .unwrap();
        let input = replay_input(code, &decoded);
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        // A prior charge proves the replay does not create a fresh budget.
        meter
            .charge_work(17, &scoop_wire::WirePath::root())
            .unwrap();
        assert_eq!(
            input.fingerprint_with_meter(&mut meter).unwrap(),
            code.fingerprint()
        );
        let work = meter.usage().validation_work_units;
        for inclusive in [true, false] {
            let mut meter = BudgetMeter::new(DecodeLimits {
                validation_work_units: work - u64::from(!inclusive),
                ..DecodeLimits::default()
            });
            meter
                .charge_work(17, &scoop_wire::WirePath::root())
                .unwrap();
            let result = input.fingerprint_with_meter(&mut meter);
            if inclusive {
                assert_eq!(result.unwrap(), code.fingerprint());
            } else {
                assert!(
                    matches!(result, Err(LayoutCodeFingerprintError::Resource(error))
                    if matches!(error.kind(), WireErrorKind::LimitExceeded {
                        resource: ResourceKind::ValidationWorkUnits, ..
                    }))
                );
            }
        }
    });
}

fn replay_input<'a>(
    code: &'a VerifiedCodeFingerprintV2,
    strong: &'a DecodedStrongProductionSectionV2,
) -> LayoutCodeFingerprintInputV1<'a, DecodedStrongProductionSectionV2> {
    LayoutCodeFingerprintInputV1 {
        objects: code.production().link_objects(),
        production: code.production().projection(),
        strong,
        link_extension_contributions: code.link_extension_contributions(),
        native_requirements: code.native_requirements(),
        native_contracts: code.native_contracts(),
        defined_symbols: code.defined_symbols(),
        undefined_symbols: code.undefined_symbols(),
    }
}
