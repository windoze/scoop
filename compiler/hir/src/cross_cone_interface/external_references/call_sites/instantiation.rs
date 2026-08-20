use scoop_identity::PersistentCallableApplicationId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// The already selected application, separate from the call's source route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirDependencyCallInstantiationV1<T = PersistentCallableApplicationId> {
    Direct,
    Application(T),
    /// A direct C call from a release body, without the provider's Scoop entry.
    NativeLeaf,
}

impl<T> HirDependencyCallInstantiationV1<T> {
    pub fn try_map<U, E>(
        self,
        map: impl FnOnce(T) -> Result<U, E>,
    ) -> Result<HirDependencyCallInstantiationV1<U>, E> {
        Ok(match self {
            Self::Direct => HirDependencyCallInstantiationV1::Direct,
            Self::NativeLeaf => HirDependencyCallInstantiationV1::NativeLeaf,
            Self::Application(id) => HirDependencyCallInstantiationV1::Application(map(id)?),
        })
    }
}

impl<T: WireEncode> WireEncode for HirDependencyCallInstantiationV1<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Direct | Self::NativeLeaf => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(if matches!(self, Self::Direct) { 1 } else { 3 })
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
            1 | 3 => 1,
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
            3 => Ok(Self::NativeLeaf),
            2 => decoder.field(1, T::decode).map(Self::Application),
            _ => unreachable!("call instantiation tag was checked"),
        }
    }
}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_identity::DecodedPersistentId;
    use scoop_wire::{decode_canonical, encode};

    type Decoded =
        HirDependencyCallInstantiationV1<DecodedPersistentId<PersistentCallableApplicationId>>;

    #[test]
    fn native_leaf_has_its_own_strict_tag() {
        let leaf: HirDependencyCallInstantiationV1 = HirDependencyCallInstantiationV1::NativeLeaf;
        assert_eq!(encode(&leaf).unwrap(), [0xa1, 0x00, 0x03]);
        assert_eq!(
            decode_canonical::<Decoded>(&[0xa1, 0x00, 0x03]).unwrap(),
            HirDependencyCallInstantiationV1::NativeLeaf
        );
        assert!(decode_canonical::<Decoded>(&[0xa2, 0x00, 0x03, 0x01, 0x00]).is_err());
        assert!(decode_canonical::<Decoded>(&[0xa1, 0x00, 0x04]).is_err());
    }
}
