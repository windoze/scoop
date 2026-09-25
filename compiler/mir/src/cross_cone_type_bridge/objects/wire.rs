use super::*;
use scoop_identity::{DecodedStrongCallableDefinitionOwner, PersistentIdResolver};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodedReadPlan {
    PublishedSingletonRoot {
        object: DecodedPersistentId<PersistentExactTypeId>,
    },
}
macro_rules! encode_read {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                match self {
                    Self::PublishedSingletonRoot { object } => {
                        tag(encoder, 2, 1)?;
                        encoder.field(1)?;
                        object.encode(encoder)
                    }
                }
            }
        }
    };
}
encode_read!(MirObjectValueReadPlanV1);
encode_read!(DecodedReadPlan);
impl WireDecode for DecodedReadPlan {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(|object| Self::PublishedSingletonRoot { object }),
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedParamFreeMirObjectValueV1 {
    value: DecodedPersistentId<PersistentObjectValueId>,
    backing: DecodedPersistentId<PersistentExactTypeId>,
    unit: DecodedPersistentId<PersistentInitializationUnitId>,
    ensure: DecodedStrongCallableDefinitionOwner,
    read: DecodedReadPlan,
}
impl DecodedParamFreeMirObjectValueV1 {
    pub fn validate(
        self,
        graph: &mut ValidatedIdentityGraph,
        types: &dyn MirTypeBridgeTypeLookupV1,
        callables: &dyn MirTypeBridgeCallableLookupV1,
    ) -> Result<ParamFreeMirObjectValueV1, MirObjectBridgeError> {
        let value = graph.resolve(self.value)?;
        let backing = graph.resolve(self.backing)?;
        let unit = graph.resolve(self.unit)?;
        let ensure = self.ensure.resolve(graph)?;
        let DecodedReadPlan::PublishedSingletonRoot { object } = self.read;
        let read = MirObjectValueReadPlanV1::PublishedSingletonRoot {
            object: graph.resolve(object)?,
        };
        ParamFreeMirObjectValueV1::try_new(
            MirObjectBridgeAuthority {
                identities: graph,
                types,
                callables,
            },
            value,
            backing,
            unit,
            ensure,
            read,
        )
    }
}
macro_rules! encode_object {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(5)?;
                encoder.field(1)?;
                self.value.encode(encoder)?;
                encoder.field(2)?;
                self.backing.encode(encoder)?;
                encoder.field(3)?;
                self.unit.encode(encoder)?;
                encoder.field(4)?;
                self.ensure.encode(encoder)?;
                encoder.field(5)?;
                self.read.encode(encoder)
            }
        }
    };
}
encode_object!(ParamFreeMirObjectValueV1);
encode_object!(DecodedParamFreeMirObjectValueV1);
impl WireDecode for DecodedParamFreeMirObjectValueV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            value: decoder.field(1, DecodedPersistentId::decode)?,
            backing: decoder.field(2, DecodedPersistentId::decode)?,
            unit: decoder.field(3, DecodedPersistentId::decode)?,
            ensure: decoder.field(4, DecodedStrongCallableDefinitionOwner::decode)?,
            read: decoder.field(5, DecodedReadPlan::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodedCause {
    ObjectValue(DecodedPersistentId<PersistentObjectValueId>),
    PropertyAccessor(DecodedPersistentId<PersistentPropertyAccessorId>),
    InitializationSupport(DecodedPersistentId<PersistentInitializationUnitId>),
}
macro_rules! encode_cause {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                match self {
                    Self::ObjectValue(value) => {
                        tag(encoder, 2, 1)?;
                        encoder.field(1)?;
                        value.encode(encoder)
                    }
                    Self::PropertyAccessor(value) => {
                        tag(encoder, 2, 2)?;
                        encoder.field(1)?;
                        value.encode(encoder)
                    }
                    Self::InitializationSupport(value) => {
                        tag(encoder, 2, 3)?;
                        encoder.field(1)?;
                        value.encode(encoder)
                    }
                }
            }
        }
    };
}
encode_cause!(MirExternalInitializationCauseV1);
encode_cause!(DecodedCause);
impl WireDecode for DecodedCause {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::ObjectValue),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::PropertyAccessor),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::InitializationSupport),
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedSelectedExternalInitializationUseV1 {
    local_unit: DecodedPersistentId<PersistentInitializationUnitId>,
    provider: DecodedPersistentId<ConeIdentity>,
    dependency_unit: DecodedPersistentId<PersistentInitializationUnitId>,
    cause: DecodedCause,
}
impl DecodedSelectedExternalInitializationUseV1 {
    pub fn validate(
        self,
        consumer: ConeIdentity,
        graph: &mut ValidatedIdentityGraph,
    ) -> Result<SelectedExternalInitializationUseV1, MirObjectBridgeError> {
        let local_unit = graph.resolve(self.local_unit)?;
        let provider = graph.resolve(self.provider)?;
        let dependency_unit = graph.resolve(self.dependency_unit)?;
        let cause = match self.cause {
            DecodedCause::ObjectValue(value) => {
                MirExternalInitializationCauseV1::ObjectValue(graph.resolve(value)?)
            }
            DecodedCause::PropertyAccessor(value) => {
                MirExternalInitializationCauseV1::PropertyAccessor(graph.resolve(value)?)
            }
            DecodedCause::InitializationSupport(value) => {
                MirExternalInitializationCauseV1::InitializationSupport(graph.resolve(value)?)
            }
        };
        SelectedExternalInitializationUseV1::try_new(
            consumer,
            graph,
            local_unit,
            provider,
            dependency_unit,
            cause,
        )
    }
}
macro_rules! encode_use {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(4)?;
                encoder.field(1)?;
                self.local_unit.encode(encoder)?;
                encoder.field(2)?;
                self.provider.encode(encoder)?;
                encoder.field(3)?;
                self.dependency_unit.encode(encoder)?;
                encoder.field(4)?;
                self.cause.encode(encoder)
            }
        }
    };
}
encode_use!(SelectedExternalInitializationUseV1);
encode_use!(DecodedSelectedExternalInitializationUseV1);
impl WireDecode for DecodedSelectedExternalInitializationUseV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            local_unit: decoder.field(1, DecodedPersistentId::decode)?,
            provider: decoder.field(2, DecodedPersistentId::decode)?,
            dependency_unit: decoder.field(3, DecodedPersistentId::decode)?,
            cause: decoder.field(4, DecodedCause::decode)?,
        })
    }
}
