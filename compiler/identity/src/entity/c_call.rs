use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// Caller protocol of one source C extern declaration. This is independent of
/// the physical C symbol's ABI and must not enter its contract merge key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CAbiCallMode {
    NativeSafe,
    GcLeaf,
}

/// Result adaptation of a C declaration, independent of its native signature.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CResultAdaptation {
    Direct,
    CaptureErrno,
}

impl WireEncode for CResultAdaptation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Direct => 1,
            Self::CaptureErrno => 2,
        })
    }
}

impl WireDecode for CResultAdaptation {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Direct),
            2 => Ok(Self::CaptureErrno),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

/// Complete native and Scoop result types at an extern boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternResult<T> {
    Direct(T),
    CaptureErrno { native: T, scoop: T },
}

impl<T> ExternResult<T> {
    pub const fn native_type(&self) -> &T {
        match self {
            Self::Direct(ty) | Self::CaptureErrno { native: ty, .. } => ty,
        }
    }

    pub const fn scoop_type(&self) -> &T {
        match self {
            Self::Direct(ty) | Self::CaptureErrno { scoop: ty, .. } => ty,
        }
    }

    pub const fn adaptation(&self) -> CResultAdaptation {
        match self {
            Self::Direct(_) => CResultAdaptation::Direct,
            Self::CaptureErrno { .. } => CResultAdaptation::CaptureErrno,
        }
    }

    pub fn map<U>(self, mut map: impl FnMut(T) -> U) -> ExternResult<U> {
        match self {
            Self::Direct(ty) => ExternResult::Direct(map(ty)),
            Self::CaptureErrno { native, scoop } => ExternResult::CaptureErrno {
                native: map(native),
                scoop: map(scoop),
            },
        }
    }
}

impl WireEncode for CAbiCallMode {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::NativeSafe => 1,
            Self::GcLeaf => 2,
        })
    }
}

impl WireDecode for CAbiCallMode {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::NativeSafe),
            2 => Ok(Self::GcLeaf),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
