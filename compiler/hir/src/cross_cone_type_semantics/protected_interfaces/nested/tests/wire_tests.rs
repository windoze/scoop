use super::*;

struct ReversedMembers<'a>(&'a ProtectedNestedSourceInterfaceV1);
impl WireEncode for ReversedMembers<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let source = self.0;
        encoder.map(9)?;
        encoder.field(1)?;
        source.kind.encode(encoder)?;
        encoder.field(2)?;
        source.modality.encode(encoder)?;
        encoder.field(3)?;
        source.type_parameters.encode(encoder)?;
        encoder.field(4)?;
        source.supertypes.encode(encoder)?;
        encoder.field(5)?;
        source.constructors.encode(encoder)?;
        encoder.field(6)?;
        wire::sequence(
            encoder,
            &source
                .members
                .values()
                .iter()
                .rev()
                .copied()
                .collect::<Vec<_>>(),
        )?;
        encoder.field(7)?;
        source.children.encode(encoder)?;
        encoder.field(8)?;
        source.source_shape.encode(encoder)?;
        encoder.field(9)?;
        source.source_support.encode(encoder)
    }
}
#[test]
fn nested_reader_rejects_noncanonical_member_order_and_unknown_tag() {
    let mut fixture = Fixture::default();
    let outer = fixture.class("Outer");
    let owner = nested_class(&mut fixture, outer, "Nested");
    let (first, first_record) =
        function(&mut fixture, owner, "first", DeclaredVisibilityV1::Public);
    let (second, second_record) =
        function(&mut fixture, owner, "second", DeclaredVisibilityV1::Private);
    let source = source(
        NominalInheritanceModalityV1::Open,
        vec![],
        vec![first, second],
        vec![],
        vec![first_record, second_record],
    );
    let bytes = encode(&ReversedMembers(&source)).unwrap();
    let decoded: DecodedProtectedNestedSourceInterfaceV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(NestedSourceResolutionError::Build(
            NestedSourceBuildError::NonCanonicalOrder
        ))
    ));

    assert!(decode_canonical::<DecodedNestedNominalSupportV1>(&[0xa1, 0, 3]).is_err());
}
