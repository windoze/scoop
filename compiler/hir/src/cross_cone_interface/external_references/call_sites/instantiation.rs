use scoop_identity::PersistentCallableApplicationId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// The already selected application, separate from the call's source route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirDependencyCallInstantiationV1<T = PersistentCallableApplicationId> {
    Direct,
    Application(T),
}

impl<T> HirDependencyCallInstantiationV1<T> {
    pub fn try_map<U, E>(
        self,
        map: impl FnOnce(T) -> Result<U, E>,
    ) -> Result<HirDependencyCallInstantiationV1<U>, E> {
        Ok(match self {
            Self::Direct => HirDependencyCallInstantiationV1::Direct,
            Self::Application(id) => HirDependencyCallInstantiationV1::Application(map(id)?),
        })
    }
}

impl<T: WireEncode> WireEncode for HirDependencyCallInstantiationV1<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Direct => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Application(id) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
        }
    }
}

impl<T: WireDecode> WireDecode for HirDependencyCallInstantiationV1<T> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(error(decoder, WireErrorKind::MissingField { field: 0 }));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 1,
            2 => 2,
            _ => return Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        };
        if fields != expected {
            return Err(error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
            ));
        }
        match tag {
            1 => Ok(Self::Direct),
            2 => decoder.field(1, T::decode).map(Self::Application),
            _ => unreachable!("call instantiation tag was checked"),
        }
    }
}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
