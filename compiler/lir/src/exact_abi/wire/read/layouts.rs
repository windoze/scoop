use super::*;

#[derive(Debug)]
pub(super) struct RawLayouts {
    receiver: RawReceiver,
    parameters: Vec<DecodedPersistentId<PersistentLayoutId>>,
    result: DecodedPersistentId<PersistentLayoutId>,
}
#[derive(Debug)]
enum RawReceiver {
    NoReceiver,
    Receiver(DecodedPersistentId<PersistentLayoutId>),
}

impl RawLayouts {
    pub(super) fn validate_against(
        self,
        expected: &CallableAbiLayoutDependenciesV1,
    ) -> Result<(), ExactCallableAbiWireError> {
        if self.parameters.len() != expected.parameters().len() {
            return Err(ExactCallableAbiWireError::Layout);
        }

        let receiver = match (self.receiver, expected.receiver()) {
            (RawReceiver::NoReceiver, CallableAbiReceiverLayoutV1::NoReceiver) => true,
            (RawReceiver::Receiver(raw), CallableAbiReceiverLayoutV1::Receiver(expected)) => {
                raw.verify(expected.identity().layout()).is_ok()
            }
            _ => false,
        };
        if !receiver
            || self
                .result
                .verify(expected.result().identity().layout())
                .is_err()
        {
            return Err(ExactCallableAbiWireError::Layout);
        }
        for (raw, expected) in self.parameters.into_iter().zip(expected.parameters()) {
            if raw.verify(expected.identity().layout()).is_err() {
                return Err(ExactCallableAbiWireError::Layout);
            }
        }
        Ok(())
    }
}
impl WireDecode for RawLayouts {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            receiver: decoder.field(1, RawReceiver::decode)?,
            parameters: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
            })?,
            result: decoder.field(3, DecodedPersistentId::decode)?,
        })
    }
}
impl WireDecode for RawReceiver {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match (fields, tag) {
            (1, 1) => Ok(Self::NoReceiver),
            (2, 2) => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Receiver),
            _ => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
impl WireEncode for RawLayouts {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.receiver.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.parameters.len() as u64)?;
        for parameter in &self.parameters {
            parameter.encode(encoder)?;
        }
        encoder.field(3)?;
        self.result.encode(encoder)
    }
}

impl WireEncode for RawReceiver {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            RawReceiver::NoReceiver => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
            }
            RawReceiver::Receiver(raw) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                raw.encode(encoder)?;
            }
        }
        Ok(())
    }
}
