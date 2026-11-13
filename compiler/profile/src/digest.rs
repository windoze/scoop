use std::fmt;

use scoop_identity::{CallableBodyKey, PersistentCallableBodyId};
use scoop_wire::Digest256;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

macro_rules! digest {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn as_array(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                Digest256::from_array(self.0).fmt(formatter)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                let bytes = value.as_bytes();
                if bytes.len() != 64 || !bytes.iter().all(u8::is_ascii_hexdigit) {
                    return Err(serde::de::Error::custom(
                        "expected a 256-bit hexadecimal value",
                    ));
                }
                let mut digest = [0; 32];
                for (index, byte) in digest.iter_mut().enumerate() {
                    *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
                        .expect("the length and ASCII hexadecimal digits were checked");
                }
                Ok(Self(digest))
            }
        }
    };
}

digest!(BodyId);
digest!(Fingerprint);

impl BodyId {
    /// Retain the compiler's existing machine-body identity, not its display name.
    pub fn from_key(key: &CallableBodyKey) -> Self {
        let id = PersistentCallableBodyId::from_key(key)
            .expect("a complete callable body key has a runtime identity");
        Self(*id.as_array())
    }
}

impl From<Digest256> for Fingerprint {
    fn from(value: Digest256) -> Self {
        Self(*value.as_array())
    }
}
