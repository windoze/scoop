use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedProtectedPropertyMutabilityV1 {
    ReadOnly,
    ReadWrite {
        setter: DecodedPersistentId<PersistentPropertyAccessorId>,
        setter_access: DecodedDeclarationAccessSourceV1,
    },
}
impl DecodedProtectedPropertyMutabilityV1 {
    pub fn resolve<R: ProtectedPropertyInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<ProtectedPropertyMutabilityV1, ProtectedPropertyResolutionError<E>> {
        use ProtectedPropertyResolutionError as Error;
        Ok(match self {
            Self::ReadOnly => ProtectedPropertyMutabilityV1::ReadOnly,
            Self::ReadWrite {
                setter,
                setter_access,
            } => ProtectedPropertyMutabilityV1::ReadWrite {
                setter: resolver.resolve(setter).map_err(Error::Identity)?,
                setter_access: setter_access.resolve(resolver).map_err(Error::Access)?,
            },
        })
    }
}
impl WireEncode for DecodedProtectedPropertyMutabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ReadOnly => wire::tag(encoder, 1, 1),
            Self::ReadWrite {
                setter,
                setter_access,
            } => {
                wire::tag(encoder, 3, 2)?;
                encoder.field(1)?;
                setter.encode(encoder)?;
                encoder.field(2)?;
                setter_access.encode(encoder)
            }
        }
    }
}
impl WireDecode for DecodedProtectedPropertyMutabilityV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let count = decoder.map()?;
        match decoder.field(0, |d| d.unsigned())? {
            1 => {
                wire::expect_fields(decoder, count, 1)?;
                Ok(Self::ReadOnly)
            }
            2 => {
                wire::expect_fields(decoder, count, 3)?;
                Ok(Self::ReadWrite {
                    setter: decoder.field(1, DecodedPersistentId::decode)?,
                    setter_access: decoder.field(2, DecodedDeclarationAccessSourceV1::decode)?,
                })
            }
            tag => Err(wire::error(
                decoder,
                scoop_wire::WireErrorKind::UnknownTag { tag },
            )),
        }
    }
}
