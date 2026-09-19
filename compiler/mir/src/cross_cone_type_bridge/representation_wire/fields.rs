use super::*;

macro_rules! field_wire {
    ($decoded:ident, $trusted:ident, $id:ty) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $decoded {
            field: DecodedPersistentId<$id>,
            value: DecodedPersistentId<PersistentExactTypeId>,
        }
        impl $decoded {
            pub(super) fn resolve(
                self,
                graph: &mut ValidatedIdentityGraph,
            ) -> Result<$trusted, MirTypeBridgeError> {
                Ok($trusted {
                    field: graph.resolve(self.field)?,
                    value: graph.resolve(self.value)?,
                })
            }
        }
        impl WireDecode for $decoded {
            fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
                decoder.expect_map(2)?;
                Ok(Self {
                    field: decoder.field(1, DecodedPersistentId::decode)?,
                    value: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
        }
        impl WireEncode for $decoded {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(2)?;
                encoder.field(1)?;
                self.field.encode(encoder)?;
                encoder.field(2)?;
                self.value.encode(encoder)
            }
        }
        impl WireEncode for $trusted {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(2)?;
                encoder.field(1)?;
                self.field.encode(encoder)?;
                encoder.field(2)?;
                self.value.encode(encoder)
            }
        }
    };
}
field_wire!(
    DecodedMirRepresentationFieldV1,
    MirRepresentationFieldV1,
    PersistentFieldId
);
field_wire!(
    DecodedMirRepresentationVariantFieldV1,
    MirRepresentationVariantFieldV1,
    PersistentEnumVariantFieldId
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedMirRepresentationVariantV1 {
    variant: DecodedPersistentId<PersistentEnumVariantId>,
    fields: Vec<DecodedMirRepresentationVariantFieldV1>,
    gc: MirGcKindV1,
}
impl DecodedMirRepresentationVariantV1 {
    pub(super) fn resolve(
        self,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<MirRepresentationVariantV1, MirTypeBridgeError> {
        Ok(MirRepresentationVariantV1 {
            variant: graph.resolve(self.variant)?,
            fields: wire::resolve_sequence(self.fields, graph, meter, |field, graph, _| {
                field.resolve(graph)
            })?,
            gc: self.gc,
        })
    }
}
impl WireDecode for DecodedMirRepresentationVariantV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            variant: decoder.field(1, DecodedPersistentId::decode)?,
            fields: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| {
                    DecodedMirRepresentationVariantFieldV1::decode(decoder)
                })
            })?,
            gc: decoder.field(3, MirGcKindV1::decode)?,
        })
    }
}
macro_rules! variant_wire {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(3)?;
                encoder.field(1)?;
                self.variant.encode(encoder)?;
                encoder.field(2)?;
                sequence(encoder, &self.fields)?;
                encoder.field(3)?;
                self.gc.encode(encoder)
            }
        }
    };
}
variant_wire!(DecodedMirRepresentationVariantV1);
variant_wire!(MirRepresentationVariantV1);
