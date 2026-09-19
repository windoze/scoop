use super::*;
use scoop_identity::{OptionalSignatureType, SourceNominalKind};
use scoop_wire::{Encoder, WireEncode};

struct RawPayload<'a> {
    payload: &'a ProtectedCallablePayloadV1,
    receiver: OptionalSignatureType,
    source: ProtectedSourceInterfaceUseV1,
}
impl WireEncode for RawPayload<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        self.payload.owner().encode(encoder)?;
        encoder.field(2)?;
        self.payload.type_parameters().encode(encoder)?;
        encoder.field(3)?;
        self.receiver.encode(encoder)?;
        encoder.field(4)?;
        self.payload.parameters().encode(encoder)?;
        encoder.field(5)?;
        self.payload.result().encode(encoder)?;
        encoder.field(6)?;
        self.payload.effects().encode(encoder)?;
        encoder.field(7)?;
        self.payload.modality().encode(encoder)?;
        encoder.field(8)?;
        self.source.encode(encoder)?;
        encoder.field(9)?;
        self.payload.slot_relations().encode(encoder)
    }
}

#[test]
fn reader_rejects_extension_receiver_and_cross_declaration_source_protocol() {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let declaration = fixture.function(owner, "method", false, vec![]);
    let other = fixture.function(owner, "other", false, vec![]);
    let payload = fixture.payload(
        owner,
        declaration,
        vec![],
        SignatureTypeKey::Nominal(nominal(fixture.unit)),
    );
    for (receiver, source, expected) in [
        (
            OptionalSignatureType::from_option(Some(SignatureTypeKey::Nominal(nominal(owner)))),
            payload.source_interface(),
            ProtectedCallableInterfaceBuildError::Receiver,
        ),
        (
            OptionalSignatureType::Absent,
            ProtectedSourceInterfaceUseV1::for_declaration(other).unwrap(),
            ProtectedCallableInterfaceBuildError::SourceInterface,
        ),
    ] {
        let decoded: DecodedProtectedCallablePayloadV1 = decode_canonical(
            &encode(&RawPayload {
                payload: &payload,
                receiver,
                source,
            })
            .unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert!(
            matches!(decoded.resolve(declaration, &mut fixture, &mut meter()), Err(ProtectedCallableInterfaceResolutionError::Interface(actual)) if actual == expected)
        );
    }
}

#[test]
fn source_validation_rejects_nonclass_owner_bad_binder_scope_and_foreign_source_key() {
    let mut fixture = Fixture::default();
    let value = fixture.graph.add("Value", SourceNominalKind::Struct, &[]);
    let owner = fixture.class("Owner");
    let declaration = fixture.function(value, "method", false, vec![]);
    let generic = fixture.function(owner, "generic", true, vec![]);
    let record = fixture.record(
        value,
        declaration,
        fixture.payload(
            value,
            declaration,
            vec![],
            SignatureTypeKey::Nominal(nominal(fixture.unit)),
        ),
    );
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    assert!(matches!(
        record.validate_source(&graph, &mut fixture, &mut meter()),
        Err(ProtectedCallableSemanticError::Owner)
    ));
    let bad_scope = fixture.record(
        owner,
        generic,
        fixture.payload(
            owner,
            generic,
            vec![],
            SignatureTypeKey::Binder { depth: 1, index: 0 },
        ),
    );
    assert!(matches!(
        bad_scope.validate_source(&graph, &mut fixture, &mut meter()),
        Err(ProtectedCallableSemanticError::Signature(_))
    ));
    let valid = fixture.function(owner, "valid", false, vec![]);
    let record = fixture.record(
        owner,
        valid,
        fixture.payload(
            owner,
            valid,
            vec![],
            SignatureTypeKey::Nominal(nominal(fixture.unit)),
        ),
    );
    let key = fixture.declarations[&declaration].clone();
    fixture.declarations.insert(valid, key);
    assert!(matches!(
        record.validate_source(&graph, &mut fixture, &mut meter()),
        Err(ProtectedCallableSemanticError::Identity)
    ));
}
