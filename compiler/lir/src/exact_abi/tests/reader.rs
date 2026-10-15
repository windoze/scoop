use super::*;

struct Modified<'a> {
    value: &'a ExactCallableAbiExportV1,
    signature: CanonicalScoopAbiFunctionSignature,
    protocol: ExactCallableProtocolV1,
}
impl WireEncode for Modified<'_> {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.value.target().encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)?;
        encoder.field(3)?;
        self.value.calling_convention().encode(encoder)?;
        encoder.field(4)?;
        self.protocol.encode(encoder)?;
        encoder.field(6)?;
        self.value.definition().encode(encoder)
    }
}

#[test]
fn reader_requires_protocol_to_match_the_saved_signature() {
    let unit: ExactLayoutExportV1 = unit().into();
    let (value, foundation) =
        fixtures::function(unit.identity().exact(), ScoopAbiReturn::UnitVoid, vec![]);
    let bytes = encode(&Modified {
        value: &value,
        signature: value.canonical_signature().clone(),
        protocol: ExactCallableProtocolV1::OrdinaryManaged,
    })
    .unwrap();
    let raw = decode_canonical::<DecodedExactCallableAbiExportV1>(&bytes).unwrap();
    let mut graph = fixtures::identities(&foundation, &[value]);
    assert!(
        raw.read(TARGET, &foundation, &mut graph)
            .unwrap_err()
            .to_string()
            .contains("protocol")
    );
}
