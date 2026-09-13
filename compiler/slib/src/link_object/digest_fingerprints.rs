//! Typed values produced by the M23 digest graph evaluators.

use std::fmt;

use scoop_wire::{Encoder, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, WireEncode};

macro_rules! typed_fingerprint {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub(crate) const fn from_array(value: [u8; 32]) -> Self {
                Self(value)
            }

            pub const fn as_array(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl WireEncode for $name {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.bytes(&self.0)
            }
        }

        impl RuntimeEncode for $name {
            fn runtime_encode(
                &self,
                encoder: &mut RuntimeEncoder,
            ) -> Result<(), RuntimeEncodeError> {
                encoder.fixed(&self.0)
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

typed_fingerprint!(ObjectDefinitionFingerprintV1);
typed_fingerprint!(StrongRegistrationFingerprintV1);
