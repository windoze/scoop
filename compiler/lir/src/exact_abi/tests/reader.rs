use super::*;
use scoop_identity::ScoopAbiValueShape;

struct Modified<'a> {
    value: &'a ExactCallableAbiExportV1,
    signature: CanonicalScoopAbiFunctionSignature,
    protocol: ExactCallableProtocolV1,
    layouts: &'a CallableAbiLayoutDependenciesV1,
}
impl WireEncode for Modified<'_> {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.value.target().encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)?;
        encoder.field(3)?;
        self.value.calling_convention().encode(encoder)?;
        encoder.field(4)?;
        self.protocol.encode(encoder)?;
        encoder.field(5)?;
        self.layouts.encode(encoder)?;
        encoder.field(6)?;
        self.value.definition().encode(encoder)
    }
}

#[test]
fn reader_rejects_storage_pass_mode_protocol_and_logical_layout_drift() {
    let value = fixtures::aggregate(false);
    let unit: ExactLayoutExportV1 = unit().into();
    let expected = fixtures::function(&value, &[&value]);
    let wrong_layouts = fixtures::function(&unit, &[&unit]);
    let canonical = expected.canonical_signature();
    let raw_storage = canonical.arguments()[0].storage();
    let scalar = CanonicalScoopStorage::new(
        raw_storage.exact_type(),
        raw_storage.byte_size(),
        raw_storage.alignment(),
        ScoopAbiValueShape::Scalar,
    );
    let wrong_signature = CanonicalScoopAbiFunctionSignature::new(
        canonical.signature().clone(),
        vec![ScoopAbiArgument::direct(scalar).unwrap()],
        ScoopAbiReturn::direct(scalar).unwrap(),
        canonical.gc_effect(),
    )
    .unwrap();
    for (signature, protocol, layouts) in [
        (
            wrong_signature,
            expected.call_protocol(),
            expected.layout_dependencies(),
        ),
        (
            canonical.clone(),
            ExactCallableProtocolV1::OrdinaryManaged,
            expected.layout_dependencies(),
        ),
        (
            canonical.clone(),
            expected.call_protocol(),
            wrong_layouts.layout_dependencies(),
        ),
        (
            CanonicalScoopAbiFunctionSignature::new(
                canonical.signature().clone(),
                canonical.arguments().to_vec(),
                ScoopAbiReturn::UnitVoid,
                canonical.gc_effect(),
            )
            .unwrap(),
            expected.call_protocol(),
            expected.layout_dependencies(),
        ),
    ] {
        let bytes = encode(&Modified {
            value: &expected,
            signature,
            protocol,
            layouts,
        })
        .unwrap();
        let raw = decode_canonical::<DecodedExactCallableAbiExportV1>(&bytes).unwrap();
        assert!(raw.validate_against(&expected).is_err());
    }
}
