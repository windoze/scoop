use super::*;

struct DefinitionWire {
    semantic: PersistentLayoutId,
    plan: ObjectDefinitionPlanId,
    symbol: PersistentSymbolRequest,
}
impl WireEncode for DefinitionWire {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(3)?;
        encoder.field(1)?;
        self.semantic.encode(encoder)?;
        encoder.field(2)?;
        self.plan.encode(encoder)?;
        encoder.field(3)?;
        self.symbol.encode(encoder)
    }
}

#[test]
fn definition_reader_checks_plan_and_complete_symbol_even_when_semantic_id_matches() {
    let expected = ExactLayoutExportV1::from(unit());
    let other = ExactLayoutExportV1::from(managed());
    let expected_definition = expected.identity().definition();
    let other_definition = other.identity().definition();
    for (plan, symbol) in [
        (
            other_definition.definition_plan(),
            expected_definition.symbol(),
        ),
        (
            expected_definition.definition_plan(),
            other_definition.symbol(),
        ),
    ] {
        reject(&expected, |raw| {
            let wire = DefinitionWire {
                semantic: expected.identity().layout(),
                plan,
                symbol,
            };
            raw.definition = decode_canonical(&encode(&wire).unwrap()).unwrap();
        });
    }
}
