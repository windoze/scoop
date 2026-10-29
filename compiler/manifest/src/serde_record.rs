//! Adapters for identity values in ordinary parsed-manifest records.

use std::collections::BTreeMap;

use scoop_identity::{
    CanonicalNativeLibraryName, ConeCoordinate, NativeLibraryKind, RequestedConeKind,
    TargetProfileId,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

use crate::DependencyCoordinateKey;

#[derive(Serialize, Deserialize)]
#[serde(remote = "RequestedConeKind")]
pub(crate) enum RequestedKind {
    Library,
    Executable,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "scoop_identity::WindowsSubsystem")]
pub(crate) enum Subsystem {
    Console,
    Gui,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "NativeLibraryKind")]
pub(crate) enum LibraryKind {
    TargetDefault,
    Dynamic,
    StaticArchive,
    Framework,
}

pub(crate) mod coordinate {
    use super::*;

    pub fn serialize<S: Serializer>(
        value: &ConeCoordinate,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        (value.group(), value.name(), value.version()).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ConeCoordinate, D::Error> {
        let (group, name, version) = <(String, String, String)>::deserialize(deserializer)?;
        ConeCoordinate::new(&group, &name, &version).map_err(D::Error::custom)
    }
}

pub(crate) mod dependencies {
    use super::*;

    pub fn serialize<S: Serializer>(
        values: &BTreeMap<DependencyCoordinateKey, ConeCoordinate>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        values
            .iter()
            .map(|(key, value)| (key, (value.group(), value.name(), value.version())))
            .collect::<BTreeMap<_, _>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<DependencyCoordinateKey, ConeCoordinate>, D::Error> {
        let values = BTreeMap::<DependencyCoordinateKey, (String, String, String)>::deserialize(
            deserializer,
        )?;
        values
            .into_iter()
            .map(|(key, (group, name, version))| {
                ConeCoordinate::new(&group, &name, &version)
                    .map(|coordinate| (key, coordinate))
                    .map_err(D::Error::custom)
            })
            .collect()
    }
}

pub(crate) mod target {
    use super::*;

    pub fn serialize<S: Serializer>(
        value: &TargetProfileId,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        (value.canonical_name(), value.major_version()).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<TargetProfileId, D::Error> {
        let (name, version) = <(String, u32)>::deserialize(deserializer)?;
        TargetProfileId::new(&name, version).map_err(D::Error::custom)
    }
}

pub(crate) mod library_name {
    use super::*;

    pub fn serialize<S: Serializer>(
        value: &CanonicalNativeLibraryName,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.as_str().serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<CanonicalNativeLibraryName, D::Error> {
        CanonicalNativeLibraryName::from_owned(String::deserialize(deserializer)?)
            .map_err(D::Error::custom)
    }
}
