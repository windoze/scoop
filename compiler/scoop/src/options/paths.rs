use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use scoop_protocol::{HostPathCarrier, HostPathError};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Empty arguments are distinct from paths; nonempty values use the host carrier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommandArgument(Option<HostPathCarrier>);

impl CommandArgument {
    pub fn from_os(value: &OsStr) -> Result<Self, HostPathError> {
        Ok(Self(if value.is_empty() {
            None
        } else {
            Some(HostPathCarrier::from_path(Path::new(value))?)
        }))
    }

    pub fn to_os(&self) -> Result<OsString, HostPathError> {
        self.0.as_ref().map_or_else(
            || Ok(OsString::new()),
            |value| value.to_path_buf().map(PathBuf::into_os_string),
        )
    }
}

pub(super) mod optional {
    use super::*;
    pub fn serialize<S: Serializer>(
        value: &Option<PathBuf>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value
            .as_deref()
            .map(HostPathCarrier::from_path)
            .transpose()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<PathBuf>, D::Error> {
        Option::<HostPathCarrier>::deserialize(deserializer)?
            .map(|path| path.to_path_buf())
            .transpose()
            .map_err(serde::de::Error::custom)
    }
}

pub(super) mod multiple {
    use super::*;
    pub fn serialize<S: Serializer>(value: &[PathBuf], serializer: S) -> Result<S::Ok, S::Error> {
        value
            .iter()
            .map(|path| HostPathCarrier::from_path(path))
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<PathBuf>, D::Error> {
        Vec::<HostPathCarrier>::deserialize(deserializer)?
            .iter()
            .map(HostPathCarrier::to_path_buf)
            .collect::<Result<_, _>>()
            .map_err(serde::de::Error::custom)
    }
}

pub(super) mod arguments {
    use super::*;
    pub fn serialize<S: Serializer>(value: &[OsString], serializer: S) -> Result<S::Ok, S::Error> {
        value
            .iter()
            .map(|arg| CommandArgument::from_os(arg))
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<OsString>, D::Error> {
        Vec::<CommandArgument>::deserialize(deserializer)?
            .iter()
            .map(CommandArgument::to_os)
            .collect::<Result<_, _>>()
            .map_err(serde::de::Error::custom)
    }
}

pub(super) mod debug_info {
    use super::*;
    pub fn serialize<S: Serializer>(
        value: &Option<scoop_lir::DebugInfoLevel>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.map(|value| value.as_str()).serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<scoop_lir::DebugInfoLevel>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .map(|value| value.parse())
            .transpose()
            .map_err(serde::de::Error::custom)
    }
}

pub(super) mod export_ir {
    use super::*;
    pub fn serialize<S: Serializer>(
        value: &scoop_protocol::IrTemplateExport,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.as_str().serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<scoop_protocol::IrTemplateExport, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}
