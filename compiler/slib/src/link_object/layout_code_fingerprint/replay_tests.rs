use super::*;
use scoop_lir::DecodedConeProductionSectionV2;
use scoop_wire::{decode_canonical, encode};

#[test]
fn original_strong_wire_recomputes_the_exact_producer_code() {
    with_empty_layout_code_fixture(|fixture| {
        let code = fixture.code.code();
        let bytes = encode(code.production().strong_production()).unwrap();
        let decoded = decode_canonical::<DecodedConeProductionSectionV2>(&bytes).unwrap();
        let input = replay_input(code, &decoded);
        assert_eq!(input.fingerprint().unwrap(), code.fingerprint());
        assert_eq!(encode(&decoded).unwrap(), bytes);
    });
}

fn replay_input<'a>(
    code: &'a VerifiedCodeFingerprintV2,
    strong: &'a DecodedConeProductionSectionV2,
) -> LayoutCodeFingerprintInputV1<'a, DecodedConeProductionSectionV2> {
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
