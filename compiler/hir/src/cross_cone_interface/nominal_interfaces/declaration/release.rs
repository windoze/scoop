use super::*;
use crate::ReleaseValueBinderV1;
use scoop_wire::WireErrorKind;

/// The provider owns a parameter-free hook; generic owners also export a body.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum NominalReleasePolicyV1 {
    #[default]
    None,
    SynchronousGcFree {
        requirements: Vec<ReleaseValueBinderV1>,
    },
}

impl NominalReleasePolicyV1 {
    pub(in super::super) fn validate(
        &self,
        shape: &NominalSourceShapeV1,
        modality: NominalInheritanceModalityV1,
        parameters: usize,
    ) -> Result<(), NominalInterfaceRecordBuildError> {
        let Self::SynchronousGcFree { requirements } = self else {
            return Ok(());
        };
        if !matches!(shape, NominalSourceShapeV1::Class(_))
            || modality != NominalInheritanceModalityV1::Final
        {
            return Err(NominalInterfaceRecordBuildError::ReleaseOwner);
        }
        for (position, binder) in requirements.iter().enumerate() {
            if binder.depth != 0
                || binder.index as usize >= parameters
                || position > 0 && requirements[position - 1] >= *binder
            {
                return Err(NominalInterfaceRecordBuildError::ReleaseConditionBinder { position });
            }
        }
        Ok(())
    }
}

impl WireEncode for NominalReleasePolicyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::None => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::SynchronousGcFree { requirements } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                encoder.array(requirements.len() as u64)?;
                for binder in requirements {
                    encoder.array(2)?;
                    encoder.unsigned(u64::from(binder.depth))?;
                    encoder.unsigned(u64::from(binder.index))?;
                }
                Ok(())
            }
        }
    }
}

impl WireDecode for NominalReleasePolicyV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let length = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 1,
            2 => 2,
            tag => return Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        };
        if length != expected {
            return Err(error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected,
                    actual: length,
                },
            ));
        }
        if tag == 1 {
            return Ok(Self::None);
        }
        let requirements = decoder.field(1, |decoder| {
            decoder.decode_array(|decoder, _| {
                let actual = decoder.array()?;
                if actual != 2 {
                    return Err(error(
                        decoder,
                        WireErrorKind::InvalidLength {
                            expected: 2,
                            actual,
                        },
                    ));
                }
                let mut component = || {
                    let value = decoder.unsigned()?;
                    u32::try_from(value)
                        .map_err(|_| error(decoder, WireErrorKind::IntegerOutOfRange))
                };
                Ok(ReleaseValueBinderV1 {
                    depth: component()?,
                    index: component()?,
                })
            })
        })?;
        Ok(Self::SynchronousGcFree { requirements })
    }
}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
