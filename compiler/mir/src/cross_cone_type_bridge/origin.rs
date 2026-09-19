use super::*;
use scoop_identity::{DecodedGeneratedNominalKey, PersistentIdResolver};

/// Generated origins retain their canonical role and are never source aliases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTypeOriginV1 {
    SourceNominal(PersistentTypeId),
    GeneratedNominal {
        nominal: PersistentTypeId,
        role: GeneratedNominalKey,
    },
}
impl MirTypeOriginV1 {
    pub const fn nominal(&self) -> PersistentTypeId {
        match self {
            Self::SourceNominal(nominal) | Self::GeneratedNominal { nominal, .. } => *nominal,
        }
    }
}
impl WireEncode for MirTypeOriginV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::SourceNominal(nominal) => {
                tag(encoder, 2, 1)?;
                encoder.field(1)?;
                nominal.encode(encoder)
            }
            Self::GeneratedNominal { nominal, role } => {
                tag(encoder, 3, 2)?;
                encoder.field(1)?;
                nominal.encode(encoder)?;
                encoder.field(2)?;
                role.encode(encoder)
            }
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedMirTypeOriginV1 {
    SourceNominal(DecodedPersistentId<PersistentTypeId>),
    GeneratedNominal {
        nominal: DecodedPersistentId<PersistentTypeId>,
        role: DecodedGeneratedNominalKey,
    },
}
impl DecodedMirTypeOriginV1 {
    pub(super) fn resolve(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<MirTypeOriginV1, MirTypeBridgeError> {
        match self {
            Self::SourceNominal(nominal) => {
                Ok(MirTypeOriginV1::SourceNominal(identities.resolve(nominal)?))
            }
            Self::GeneratedNominal { nominal, role } => {
                let nominal = identities.resolve(nominal)?;
                if matches!(
                    role,
                    DecodedGeneratedNominalKey::ClosureEnvironment { .. }
                        | DecodedGeneratedNominalKey::CallableAdapterEnvironment { .. }
                        | DecodedGeneratedNominalKey::CoroutineFrame { .. }
                        | DecodedGeneratedNominalKey::ContinuationAdapterEnvironment { .. }
                ) {
                    return Err(MirTypeBridgeError::GeneratedExecutionShapeGate { nominal });
                }
                let canonical = identities.canonical_key::<_, GeneratedNominalKey>(nominal)?;
                let role = role
                    .resolve(identities)
                    .map_err(|error| MirTypeBridgeError::GeneratedRoleReference(Box::new(error)))?;
                if canonical.as_ref() != &role {
                    return Err(MirTypeBridgeError::GeneratedRoleMismatch { nominal });
                }
                Ok(MirTypeOriginV1::GeneratedNominal {
                    nominal,
                    role: canonical.as_ref().clone(),
                })
            }
        }
    }
}
impl WireDecode for DecodedMirTypeOriginV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let count = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                fields(decoder, count, 2)?;
                Ok(Self::SourceNominal(
                    decoder.field(1, DecodedPersistentId::decode)?,
                ))
            }
            2 => {
                fields(decoder, count, 3)?;
                Ok(Self::GeneratedNominal {
                    nominal: decoder.field(1, DecodedPersistentId::decode)?,
                    role: decoder.field(2, DecodedGeneratedNominalKey::decode)?,
                })
            }
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
impl WireEncode for DecodedMirTypeOriginV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::SourceNominal(nominal) => {
                tag(encoder, 2, 1)?;
                encoder.field(1)?;
                nominal.encode(encoder)
            }
            Self::GeneratedNominal { nominal, role } => {
                tag(encoder, 3, 2)?;
                encoder.field(1)?;
                nominal.encode(encoder)?;
                encoder.field(2)?;
                role.encode(encoder)
            }
        }
    }
}
