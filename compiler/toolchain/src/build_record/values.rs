//! Existing host path and digest representations for local JSON records.
use scoop_protocol::HostPathCarrier;
use scoop_wire::Digest256;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::{Path, PathBuf};

pub(crate) mod path {
    use super::*;
    pub fn serialize<S: Serializer>(value: &Path, serializer: S) -> Result<S::Ok, S::Error> {
        HostPathCarrier::from_path(value)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(decoder: D) -> Result<PathBuf, D::Error> {
        HostPathCarrier::deserialize(decoder)?
            .to_path_buf()
            .map_err(serde::de::Error::custom)
    }
}

pub(crate) mod paths {
    use super::*;
    pub fn serialize<S: Serializer>(values: &[PathBuf], serializer: S) -> Result<S::Ok, S::Error> {
        values
            .iter()
            .map(|p| HostPathCarrier::from_path(p))
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(decoder: D) -> Result<Vec<PathBuf>, D::Error> {
        Vec::<HostPathCarrier>::deserialize(decoder)?
            .iter()
            .map(|p| p.to_path_buf().map_err(serde::de::Error::custom))
            .collect()
    }
}

pub(crate) mod digest {
    use super::*;
    pub fn serialize<S: Serializer>(value: &Digest256, serializer: S) -> Result<S::Ok, S::Error> {
        value.as_array().serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(decoder: D) -> Result<Digest256, D::Error> {
        <[u8; 32]>::deserialize(decoder).map(Digest256::from_array)
    }
}
