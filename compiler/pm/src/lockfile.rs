//! Portable exact package choices. Local source locations live in prepare results.

use scoop_identity::{ConeCoordinate, RequestedConeKind};
use scoop_manifest::package::RegistryUrl;
use serde::{Deserialize, Serialize};

mod store;
mod validate;
pub use store::LockTransaction;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Purpose {
    #[default]
    Production,
    Test,
}

impl std::fmt::Display for Purpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Production => "production",
            Self::Test => "test",
        })
    }
}

impl std::str::FromStr for Purpose {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "production" => Ok(Self::Production),
            "test" => Ok(Self::Test),
            _ => Err("purpose must be production or test".into()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lockfile {
    pub format: u32,
    pub resolutions: Vec<LockedResolution>,
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            format: 1,
            resolutions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct LockedResolution {
    pub root: String,
    pub purpose: Purpose,
    pub target: String,
    #[serde(with = "scoop_manifest::package::format::coordinate")]
    pub root_coordinate: ConeCoordinate,
    pub nodes: Vec<LockedNode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct LockedNode {
    #[serde(with = "scoop_manifest::package::format::coordinate")]
    pub coordinate: ConeCoordinate,
    #[serde(with = "scoop_manifest::package::format::Kind")]
    pub kind: RequestedConeKind,
    pub source: LockedSource,
    #[serde(with = "scoop_manifest::package::format::coordinates")]
    pub dependencies: Vec<ConeCoordinate>,
    #[serde(with = "scoop_manifest::package::format::coordinates")]
    pub test_dependencies: Vec<ConeCoordinate>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum LockedSource {
    Path {
        path: String,
    },
    Git {
        git: String,
        commit: String,
        subdir: String,
    },
    Registry {
        registry: RegistryUrl,
        archive: String,
        sha256: String,
    },
    Artifact {
        path: String,
    },
    Sysroot {
        name: String,
    },
}

impl std::fmt::Display for LockedSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path { path } => write!(f, "path {path}"),
            Self::Git {
                git,
                commit,
                subdir,
            } => write!(f, "git {git}#{commit}:{subdir}"),
            Self::Registry { registry, .. } => write!(f, "registry {}", registry.as_str()),
            Self::Artifact { path } => write!(f, "artifact {path}"),
            Self::Sysroot { name } => write!(f, "sysroot {name}"),
        }
    }
}

#[cfg(test)]
mod tests;
