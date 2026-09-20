use super::*;

impl WireDecode for DecodedContract {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(8)?;
        Ok(Self {
            owner: decoder.field(1, DecodedSourceNominalId::decode)?,
            modality: decoder.field(2, NominalInheritanceModalityV1::decode)?,
            type_parameters: decoder.field(3, DecodedCanonicalBinderListV1::decode)?,
            supertypes: decoder.field(4, DecodedCanonicalSignatureTypesV1::decode)?,
            constructors: decoder
                .field(5, |d| d.decode_array(|d, _| DecodedPersistentId::decode(d)))?,
            members: decoder.field(6, |d| {
                d.decode_array(|d, _| DecodedNestedSourceMemberRefV1::decode(d))
            })?,
            children: decoder.field(7, |d| {
                d.decode_array(|d, _| DecodedSourceNominalId::decode(d))
            })?,
            source_shape: decoder.field(8, DecodedNominalSourceShapeV1::decode)?,
        })
    }
}
impl WireEncode for DecodedContract {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.modality.encode(encoder)?;
        encoder.field(3)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(4)?;
        self.supertypes.encode(encoder)?;
        encoder.field(5)?;
        super::super::super::wire::sequence(encoder, &self.constructors)?;
        encoder.field(6)?;
        super::super::super::wire::sequence(encoder, &self.members)?;
        encoder.field(7)?;
        super::super::super::wire::sequence(encoder, &self.children)?;
        encoder.field(8)?;
        self.source_shape.encode(encoder)
    }
}
impl WireEncode for NominalSourceContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.modality.encode(encoder)?;
        encoder.field(3)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(4)?;
        self.supertypes.encode(encoder)?;
        encoder.field(5)?;
        self.constructors.encode(encoder)?;
        encoder.field(6)?;
        self.members.encode(encoder)?;
        encoder.field(7)?;
        self.children.encode(encoder)?;
        encoder.field(8)?;
        self.source_shape.encode(encoder)
    }
}
