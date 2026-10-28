//! Preserve the existing structured diagnostic representation across the PM boundary.

use serde::{Deserialize, Serialize};

pub fn serialize<S: serde::Serializer>(
    value: &crate::StructuredDiagnosticV1,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    scoop_wire::encode(value)
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}

pub fn deserialize<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<crate::StructuredDiagnosticV1, D::Error> {
    let bytes = Vec::<u8>::deserialize(deserializer)?;
    scoop_wire::decode_canonical(&bytes).map_err(serde::de::Error::custom)
}
