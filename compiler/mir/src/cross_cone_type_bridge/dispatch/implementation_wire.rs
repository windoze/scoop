use super::*;
use scoop_identity::{DecodedDispatchDeclarationOwner, DecodedStrongCallableDefinitionOwner};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DecodedImplementation {
    AbstractObligation {
        declaration: DecodedDispatchDeclarationOwner,
        trap_target: DecodedStrongCallableDefinitionOwner,
        receiver: MirDispatchReceiverAdaptationV1,
    },
    DirectStrongTarget {
        target: DecodedStrongCallableDefinitionOwner,
        receiver: MirDispatchReceiverAdaptationV1,
    },
    InterfaceDefaultTarget {
        target: DecodedStrongCallableDefinitionOwner,
        receiver: MirDispatchReceiverAdaptationV1,
    },
    AdjustThunkTarget(DecodedStrongCallableDefinitionOwner),
}
impl DecodedImplementation {
    pub(super) fn resolve(
        self,
        graph: &mut ValidatedIdentityGraph,
    ) -> Result<MirDispatchImplementationV1, MirDispatchSchemaError> {
        Ok(match self {
            Self::AbstractObligation {
                declaration,
                trap_target,
                receiver,
            } => MirDispatchImplementationV1::AbstractObligation {
                declaration: declaration.resolve(graph)?,
                trap_target: trap_target.resolve(graph)?,
                receiver,
            },
            Self::DirectStrongTarget { target, receiver } => {
                MirDispatchImplementationV1::DirectStrongTarget {
                    target: target.resolve(graph)?,
                    receiver,
                }
            }
            Self::InterfaceDefaultTarget { target, receiver } => {
                MirDispatchImplementationV1::InterfaceDefaultTarget {
                    target: target.resolve(graph)?,
                    receiver,
                }
            }
            Self::AdjustThunkTarget(target) => {
                MirDispatchImplementationV1::AdjustThunkTarget(target.resolve(graph)?)
            }
        })
    }
}
macro_rules! encode_implementation {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                match self {
                    Self::AbstractObligation {
                        declaration,
                        trap_target,
                        receiver,
                    } => {
                        tag(encoder, 4, 1)?;
                        encoder.field(1)?;
                        declaration.encode(encoder)?;
                        encoder.field(2)?;
                        trap_target.encode(encoder)?;
                        encoder.field(3)?;
                        receiver.encode(encoder)
                    }
                    Self::DirectStrongTarget { target, receiver }
                    | Self::InterfaceDefaultTarget { target, receiver } => {
                        tag(
                            encoder,
                            3,
                            if matches!(self, Self::DirectStrongTarget { .. }) {
                                2
                            } else {
                                3
                            },
                        )?;
                        encoder.field(1)?;
                        target.encode(encoder)?;
                        encoder.field(2)?;
                        receiver.encode(encoder)
                    }
                    Self::AdjustThunkTarget(target) => {
                        tag(encoder, 2, 4)?;
                        encoder.field(1)?;
                        target.encode(encoder)
                    }
                }
            }
        }
    };
}
encode_implementation!(MirDispatchImplementationV1);
encode_implementation!(DecodedImplementation);
impl WireDecode for DecodedImplementation {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let count = decoder.map()?;
        let value = decoder.field(0, Decoder::unsigned)?;
        fields(
            decoder,
            count,
            match value {
                1 => 4,
                4 => 2,
                _ => 3,
            },
        )?;
        Ok(match value {
            1 => Self::AbstractObligation {
                declaration: decoder.field(1, DecodedDispatchDeclarationOwner::decode)?,
                trap_target: decoder.field(2, DecodedStrongCallableDefinitionOwner::decode)?,
                receiver: decoder.field(3, MirDispatchReceiverAdaptationV1::decode)?,
            },
            2 | 3 => {
                let target = decoder.field(1, DecodedStrongCallableDefinitionOwner::decode)?;
                let receiver = decoder.field(2, MirDispatchReceiverAdaptationV1::decode)?;
                if value == 2 {
                    Self::DirectStrongTarget { target, receiver }
                } else {
                    Self::InterfaceDefaultTarget { target, receiver }
                }
            }
            4 => Self::AdjustThunkTarget(
                decoder.field(1, DecodedStrongCallableDefinitionOwner::decode)?,
            ),
            tag => return Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        })
    }
}
impl WireEncode for MirDispatchReceiverAdaptationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        tag(
            encoder,
            1,
            match self {
                Self::Identity => 1,
                Self::ReferenceDispatch => 2,
            },
        )
    }
}
impl WireDecode for MirDispatchReceiverAdaptationV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::Identity),
            2 => Ok(Self::ReferenceDispatch),
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
