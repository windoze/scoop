use super::*;

impl WireEncode for InstanceRepresentationV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        match self.kind() {
            InstanceRepresentationKindV1::ClassObject(value) => {
                sum(encoder, 1, 3)?;
                encoder.field(1)?;
                match value.base_prefix() {
                    crate::ClassBasePrefixV1::NoBase => sum(encoder, 1, 0)?,
                    crate::ClassBasePrefixV1::BasePrefix {
                        exact,
                        layout,
                        byte_size,
                        alignment,
                    } => {
                        sum(encoder, 2, 4)?;
                        field(encoder, 1, &exact)?;
                        field(encoder, 2, &layout)?;
                        unsigned(encoder, 3, byte_size)?;
                        unsigned(encoder, 4, alignment.get())?;
                    }
                }
                encoder.field(2)?;
                nominal_fields(encoder, value.declared_fields())?;
                encoder.field(3)?;
                nominal_fields(encoder, value.complete_fields())
            }
            InstanceRepresentationKindV1::BoxedPayload(payload) => {
                sum(encoder, 2, 2)?;
                field(encoder, 1, &payload.exact())?;
                field(encoder, 2, &payload.layout())
            }
            InstanceRepresentationKindV1::InlineBytes => sum(encoder, 3, 0),
            InstanceRepresentationKindV1::InlineArray { element, storage } => {
                sum(encoder, 4, 2)?;
                field(encoder, 1, &element.exact())?;
                field(encoder, 2, storage)
            }
            InstanceRepresentationKindV1::AbstractReference => sum(encoder, 5, 0),
        }
    }
}
