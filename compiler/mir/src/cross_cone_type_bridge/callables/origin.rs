use super::*;
use scoop_identity::{DecodedGeneratedCallableKey, PersistentIdResolver};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirCallableOriginV1 {
    Application(scoop_identity::PersistentCallableApplicationId),
    Function(PersistentFunctionId),
    Constructor(PersistentConstructorId),
    Accessor(PersistentPropertyAccessorId),
    Generated {
        callable: PersistentGeneratedCallableId,
        role: GeneratedCallableKey,
    },
}
impl MirCallableOriginV1 {
    pub const fn implementation(&self) -> Option<StrongCallableDefinitionOwner> {
        Some(match self {
            Self::Application(_) => return None,
            Self::Function(id) => StrongCallableDefinitionOwner::Function(*id),
            Self::Constructor(id) => StrongCallableDefinitionOwner::Constructor(*id),
            Self::Accessor(id) => StrongCallableDefinitionOwner::PropertyAccessor(*id),
            Self::Generated { callable, .. } => {
                StrongCallableDefinitionOwner::GeneratedCallable(*callable)
            }
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedMirCallableOriginV1 {
    Application(DecodedPersistentId<scoop_identity::PersistentCallableApplicationId>),
    Function(DecodedPersistentId<PersistentFunctionId>),
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    Accessor(DecodedPersistentId<PersistentPropertyAccessorId>),
    Generated {
        callable: DecodedPersistentId<PersistentGeneratedCallableId>,
        role: DecodedGeneratedCallableKey,
    },
}
impl DecodedMirCallableOriginV1 {
    pub(super) fn resolve(
        self,
        graph: &mut ValidatedIdentityGraph,
    ) -> Result<MirCallableOriginV1, MirCallableBridgeError> {
        Ok(match self {
            Self::Application(id) => MirCallableOriginV1::Application(graph.resolve(id)?),
            Self::Function(id) => MirCallableOriginV1::Function(graph.resolve(id)?),
            Self::Constructor(id) => MirCallableOriginV1::Constructor(graph.resolve(id)?),
            Self::Accessor(id) => MirCallableOriginV1::Accessor(graph.resolve(id)?),
            Self::Generated { callable, role } => {
                let callable = graph.resolve(callable)?;
                if !matches!(
                    role,
                    DecodedGeneratedCallableKey::StaticNoGcCallbackStorageBridge { .. }
                        | DecodedGeneratedCallableKey::CoroutineStart { .. }
                        | DecodedGeneratedCallableKey::Initialization { .. }
                        | DecodedGeneratedCallableKey::ZeroArgumentConstructorAdapter { .. }
                        | DecodedGeneratedCallableKey::DerivedEquality { .. }
                        | DecodedGeneratedCallableKey::DispatchAdjust { .. }
                        | DecodedGeneratedCallableKey::BoxingAdjust { .. }
                ) {
                    return Err(MirCallableBridgeError::GeneratedExecutionGate { callable });
                }
                let role = role.resolve(graph).map_err(|error| {
                    MirCallableBridgeError::GeneratedRoleReference(Box::new(error))
                })?;
                MirCallableOriginV1::Generated { callable, role }
            }
        })
    }
}
macro_rules! encode_origin {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                match self {
                    Self::Application(id) => {
                        tag(encoder, 2, 5)?;
                        encoder.field(1)?;
                        id.encode(encoder)
                    }
                    Self::Function(id) => {
                        tag(encoder, 2, 1)?;
                        encoder.field(1)?;
                        id.encode(encoder)
                    }
                    Self::Constructor(id) => {
                        tag(encoder, 2, 2)?;
                        encoder.field(1)?;
                        id.encode(encoder)
                    }
                    Self::Accessor(id) => {
                        tag(encoder, 2, 3)?;
                        encoder.field(1)?;
                        id.encode(encoder)
                    }
                    Self::Generated { callable, role } => {
                        tag(encoder, 3, 4)?;
                        encoder.field(1)?;
                        callable.encode(encoder)?;
                        encoder.field(2)?;
                        role.encode(encoder)
                    }
                }
            }
        }
    };
}
encode_origin!(MirCallableOriginV1);
encode_origin!(DecodedMirCallableOriginV1);
impl WireDecode for DecodedMirCallableOriginV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let count = decoder.map()?;
        let kind = decoder.field(0, Decoder::unsigned)?;
        fields(decoder, count, if kind == 4 { 3 } else { 2 })?;
        match kind {
            5 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Application),
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Constructor),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Accessor),
            4 => Ok(Self::Generated {
                callable: decoder.field(1, DecodedPersistentId::decode)?,
                role: decoder.field(2, DecodedGeneratedCallableKey::decode)?,
            }),
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
