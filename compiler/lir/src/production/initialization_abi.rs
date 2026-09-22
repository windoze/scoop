//! Optional initialization-role data encoded with the shared callable ABI record.

use crate::{
    CallableAbiRecordV1, CallableAbiValidationError, DecodedCallableAbiRecordV1,
    OdrFreeLirFoundation, StrongObjectSymbolSurfaceV1,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

pub(crate) fn encode_initialization_abi(
    abi: Option<&impl WireEncode>,
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(u64::from(abi.is_some()))?;
    if let Some(abi) = abi {
        abi.encode(encoder)?;
    }
    Ok(())
}

pub(crate) fn decode_initialization_abi(
    decoder: &mut Decoder<'_, '_>,
) -> Result<Option<Box<DecodedCallableAbiRecordV1>>, WireError> {
    match decoder.array()? {
        0 => Ok(None),
        1 => decoder
            .index(0, DecodedCallableAbiRecordV1::decode)
            .map(Box::new)
            .map(Some),
        actual => Err(WireError::new(
            WireErrorKind::InvalidLength {
                expected: 1,
                actual,
            },
            decoder.path().clone(),
            Some(decoder.position()),
        )),
    }
}

pub(crate) fn validate_initialization_abi(
    abi: Option<&CallableAbiRecordV1>,
    foundation: &OdrFreeLirFoundation,
    definitions: &StrongObjectSymbolSurfaceV1,
) -> Result<(), CallableAbiValidationError> {
    if let Some(abi) = abi {
        abi.validate_against(foundation, definitions)?;
    }
    Ok(())
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
pub(crate) use test_support::initialization_cycle_abi_for_test;
#[cfg(test)]
mod tests;
