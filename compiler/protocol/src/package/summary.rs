//! Reuse the artifact summary's existing canonical representation in JSON.

use super::*;

pub fn serialize<S: serde::Serializer>(
    value: &ArtifactManifestSummaryV1,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    scoop_wire::encode(value)
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}

pub fn deserialize<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Box<ArtifactManifestSummaryV1>, D::Error> {
    let bytes = Vec::<u8>::deserialize(deserializer)?;
    scoop_wire::decode_canonical(&bytes)
        .map(Box::new)
        .map_err(serde::de::Error::custom)
}
