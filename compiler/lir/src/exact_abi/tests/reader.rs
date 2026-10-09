use super::*;
use scoop_identity::ScoopAbiValueShape;

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
fn reader_rejects_storage_pass_mode_protocol_and_logical_type_drift() {
    let value = fixtures::aggregate(false);
    let unit: ExactLayoutExportV1 = unit().into();
    let storage = value.value_handle().unwrap().canonical_storage();
    let expected = fixtures::function(
        value.identity().exact(),
        ScoopAbiReturn::direct_parts(storage, fixtures::integer_part(storage, 8)).unwrap(),
        vec![ScoopAbiArgument::direct_parts(storage, fixtures::integer_part(storage, 64)).unwrap()],
    );
    let wrong_layouts = fixtures::function(
        unit.identity().exact(),
        ScoopAbiReturn::UnitVoid,
        vec![
            ScoopAbiArgument::elided_zst(unit.value_handle().unwrap().canonical_storage()).unwrap(),
        ],
    );
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
    for (signature, protocol) in [
        (wrong_signature, expected.call_protocol()),
        (canonical.clone(), ExactCallableProtocolV1::OrdinaryManaged),
        (
            wrong_layouts.canonical_signature().clone(),
            expected.call_protocol(),
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
        ),
    ] {
        let bytes = encode(&Modified {
            value: &expected,
            signature,
            protocol,
        })
        .unwrap();
        let raw = decode_canonical::<DecodedExactCallableAbiExportV1>(&bytes).unwrap();
        assert!(raw.validate_against(&expected).is_err());
    }
}
