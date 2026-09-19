use super::*;
use scoop_wire::WireErrorKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedProtectedParameterCallingV1 {
    Required,
    Default {
        template_index: ProtectedDefaultTemplateIndexV1,
    },
    VarargEmpty {
        element_type: DecodedSignatureTypeKey,
    },
    VarargDefault {
        element_type: DecodedSignatureTypeKey,
        template_index: ProtectedDefaultTemplateIndexV1,
    },
}
impl DecodedProtectedParameterCallingV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        keys: &ProtectedDefaultKeyIndexV1,
        meter: &mut BudgetMeter,
    ) -> Result<ProtectedParameterCallingV1, ProtectedSourceResolutionError<E>> {
        use ProtectedSourceResolutionError as Error;
        meter
            .charge_nodes(1, &WirePath::root())
            .map_err(Error::Resource)?;
        Ok(match self {
            Self::Required => ProtectedParameterCallingV1::Required,
            Self::Default { template_index } => ProtectedParameterCallingV1::Default {
                template: keys.key(template_index).map_err(Error::Build)?,
            },
            Self::VarargEmpty { element_type } => {
                element_type
                    .charge_resolution(meter)
                    .map_err(Error::Resource)?;
                ProtectedParameterCallingV1::VarargEmpty {
                    element_type: element_type.resolve(resolver).map_err(Error::Foundation)?,
                }
            }
            Self::VarargDefault {
                element_type,
                template_index,
            } => {
                element_type
                    .charge_resolution(meter)
                    .map_err(Error::Resource)?;
                ProtectedParameterCallingV1::VarargDefault {
                    element_type: element_type.resolve(resolver).map_err(Error::Foundation)?,
                    template: keys.key(template_index).map_err(Error::Build)?,
                }
            }
        })
    }
}
impl WireDecode for DecodedProtectedParameterCallingV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                wire::expect_fields(decoder, fields, 1)?;
                Ok(Self::Required)
            }
            2 => {
                wire::expect_fields(decoder, fields, 2)?;
                Ok(Self::Default {
                    template_index: decoder.field(1, ProtectedDefaultTemplateIndexV1::decode)?,
                })
            }
            3 => {
                wire::expect_fields(decoder, fields, 2)?;
                Ok(Self::VarargEmpty {
                    element_type: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                })
            }
            4 => {
                wire::expect_fields(decoder, fields, 3)?;
                Ok(Self::VarargDefault {
                    element_type: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                    template_index: decoder.field(2, ProtectedDefaultTemplateIndexV1::decode)?,
                })
            }
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
impl WireEncode for DecodedProtectedParameterCallingV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Required => wire::tag(encoder, 1, 1),
            Self::Default { template_index } => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                template_index.encode(encoder)
            }
            Self::VarargEmpty { element_type } => {
                wire::tag(encoder, 2, 3)?;
                encoder.field(1)?;
                element_type.encode(encoder)
            }
            Self::VarargDefault {
                element_type,
                template_index,
            } => {
                wire::tag(encoder, 3, 4)?;
                encoder.field(1)?;
                element_type.encode(encoder)?;
                encoder.field(2)?;
                template_index.encode(encoder)
            }
        }
    }
}
