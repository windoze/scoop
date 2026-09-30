//! Runtime subtype relations carried by immutable type descriptors.

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// `T` is a complete type operand. An optional descriptor operand explicitly
/// denotes `Any` when absent; it never denotes an unresolved type.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum TypeDescriptorRelations<T> {
    #[default]
    Absent,
    Signature {
        is_suspend: bool,
        parameters: Vec<T>,
        result: T,
    },
    Interface {
        parents: Vec<T>,
    },
}

impl<T> TypeDescriptorRelations<T> {
    pub const fn runtime_kind(&self) -> u32 {
        match self {
            Self::Absent => 0,
            Self::Signature {
                is_suspend: false, ..
            } => 1,
            Self::Signature {
                is_suspend: true, ..
            } => 2,
            Self::Interface { .. } => 3,
        }
    }

    pub fn related_types(&self) -> &[T] {
        match self {
            Self::Absent => &[],
            Self::Signature { parameters, .. } => parameters,
            Self::Interface { parents } => parents,
        }
    }

    pub const fn result(&self) -> Option<&T> {
        match self {
            Self::Absent | Self::Interface { .. } => None,
            Self::Signature { result, .. } => Some(result),
        }
    }

    pub fn operands(&self) -> impl Iterator<Item = &T> {
        self.related_types().iter().chain(self.result())
    }

    pub fn try_map<R, E>(
        self,
        mut map: impl FnMut(T) -> Result<R, E>,
    ) -> Result<TypeDescriptorRelations<R>, E> {
        Ok(match self {
            Self::Absent => TypeDescriptorRelations::Absent,
            Self::Interface { parents } => TypeDescriptorRelations::Interface {
                parents: parents
                    .into_iter()
                    .map(&mut map)
                    .collect::<Result<_, _>>()?,
            },
            Self::Signature {
                is_suspend,
                parameters,
                result,
            } => TypeDescriptorRelations::Signature {
                is_suspend,
                parameters: parameters
                    .into_iter()
                    .map(&mut map)
                    .collect::<Result<_, _>>()?,
                result: map(result)?,
            },
        })
    }

    pub fn encode_with(
        &self,
        encoder: &mut Encoder,
        mut encode_type: impl FnMut(&T, &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError>,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(match self {
            Self::Absent => 1,
            Self::Signature { .. } => 3,
            Self::Interface { .. } => 2,
        })?;
        encoder.field(0)?;
        encoder.unsigned(u64::from(self.runtime_kind()))?;
        if !matches!(self, Self::Absent) {
            encoder.field(1)?;
            encoder.array(self.related_types().len() as u64)?;
            for parameter in self.related_types() {
                encode_type(parameter, encoder)?;
            }
        }
        if let Self::Signature { result, .. } = self {
            encoder.field(2)?;
            encode_type(result, encoder)?;
        }
        Ok(())
    }
}

impl<T: WireEncode> WireEncode for TypeDescriptorRelations<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.encode_with(encoder, T::encode)
    }
}

impl<T: WireDecode> WireDecode for TypeDescriptorRelations<T> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let actual = decoder.map()?;
        let kind = decoder.field(0, Decoder::unsigned)?;
        let expected = match kind {
            0 => 1,
            1 | 2 => 3,
            3 => 2,
            tag => {
                return Err(WireError::new(
                    WireErrorKind::UnknownTag { tag },
                    decoder.path().clone(),
                    Some(decoder.position()),
                ));
            }
        };
        if actual != expected {
            return Err(WireError::new(
                WireErrorKind::InvalidLength { expected, actual },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        if kind == 0 {
            return Ok(Self::Absent);
        }
        let parameters = decoder.field(1, |decoder| {
            let count = decoder.array()?;
            let capacity = u32::try_from(count)
                .ok()
                .and_then(|count| usize::try_from(count).ok())
                .ok_or_else(|| {
                    WireError::new(
                        WireErrorKind::IntegerOutOfRange,
                        decoder.path().clone(),
                        Some(decoder.position()),
                    )
                })?;
            let mut values = Vec::new();
            scoop_wire::allocation::try_reserve(&mut values, capacity, decoder.path())?;
            for index in 0..count {
                values.push(decoder.index(index, T::decode)?);
            }
            Ok(values)
        })?;
        if kind == 3 {
            return Ok(Self::Interface {
                parents: parameters,
            });
        }
        let result = decoder.field(2, T::decode)?;
        Ok(Self::Signature {
            is_suspend: kind == 2,
            parameters,
            result,
        })
    }
}

#[cfg(test)]
mod tests;
