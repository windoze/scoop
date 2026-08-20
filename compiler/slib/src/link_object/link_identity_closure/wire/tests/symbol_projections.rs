use super::*;

#[test]
fn borrowed_symbol_projection_replays_actual_owners_and_preserves_wire() {
    let expected = closure();
    let bytes = encode(&expected).unwrap();
    let mut wire = decode_canonical::<DecodedLinkIdentityClosureSectionV1>(&bytes).unwrap();
    wire.replay_symbol_projections(&expected.defined_symbols, &expected.undefined_symbols)
        .unwrap();
    assert_eq!(encode(&wire).unwrap(), bytes);
    wire.defined_symbols = decode_canonical(&[0x80]).unwrap();
    assert!(matches!(
        wire.replay_symbol_projections(&expected.defined_symbols, &expected.undefined_symbols),
        Err(LinkSymbolProjectionValidationError::DefinedSymbols(
            DefinedLinkSymbolOwnerValidationError::ProjectionMismatch
        ))
    ));
}
