use std::fmt;

use scoop_wire::{Encoder, WireEncode};

macro_rules! typed_fingerprint {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn as_array(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl WireEncode for $name {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.bytes(&self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for byte in self.0 {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }
    };
}

typed_fingerprint!(HirFingerprint);
typed_fingerprint!(MirFingerprint);
typed_fingerprint!(LirFingerprint);
typed_fingerprint!(CodeFingerprint);
typed_fingerprint!(RuntimeImageFingerprint);
typed_fingerprint!(ArtifactFingerprint);

impl HirFingerprint {
    pub(crate) const fn from_array(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl MirFingerprint {
    pub(crate) const fn from_array(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl LirFingerprint {
    pub(crate) const fn from_array(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl RuntimeImageFingerprint {
    pub(crate) const fn from_array(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl ArtifactFingerprint {
    pub(crate) const fn from_array(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FingerprintAvailability<T> {
    Unavailable,
    Available(T),
}

impl<T: WireEncode> WireEncode for FingerprintAvailability<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Unavailable => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Available(fingerprint) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                fingerprint.encode(encoder)
            }
        }
    }
}
