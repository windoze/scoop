//! Text adapters for package-manager JSON and TOML records.

use scoop_identity::{ConeCoordinate, RequestedConeKind};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

pub fn parse_coordinate(text: &str) -> Result<ConeCoordinate, String> {
    let mut parts = text.splitn(3, ':');
    let (Some(group), Some(name), Some(version)) = (parts.next(), parts.next(), parts.next())
    else {
        return Err(format!(
            "invalid package coordinate {text:?}; expected group:name:version"
        ));
    };
    ConeCoordinate::new(group, name, version).map_err(|error| error.to_string())
}

pub mod coordinate {
    use super::*;

    pub fn serialize<S: Serializer>(
        value: &ConeCoordinate,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ConeCoordinate, D::Error> {
        parse_coordinate(&String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

pub mod coordinates {
    use super::*;

    pub fn serialize<S: Serializer>(
        values: &[ConeCoordinate],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<ConeCoordinate>, D::Error> {
        Vec::<String>::deserialize(deserializer)?
            .iter()
            .map(|text| parse_coordinate(text).map_err(D::Error::custom))
            .collect()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "RequestedConeKind", rename_all = "lowercase")]
pub enum Kind {
    Library,
    Executable,
}
