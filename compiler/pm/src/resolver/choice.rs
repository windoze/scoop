use std::fmt;

use super::*;

/// Source identity is part of the choice domain, not a second package variable.
/// The selector and registry download location do not create distinct sources.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Source {
    First,
    Path(String),
    Git(String, String, String),
    Registry(String),
    Artifact(String),
    Sysroot(String),
    Last,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct Choice {
    pub version: semver::Version,
    source: Source,
}

impl Choice {
    pub fn any_source(version: &str) -> pubgrub::Ranges<Self> {
        let version = semver::Version::parse(version).expect("coordinate has a valid version");
        pubgrub::Ranges::from_range_bounds(
            Self {
                version: version.clone(),
                source: Source::First,
            }..=Self {
                version,
                source: Source::Last,
            },
        )
    }
    pub fn new(coordinate: &ConeCoordinate, source: &LockedSource) -> Self {
        let source = match source {
            LockedSource::Path { path } => Source::Path(path.clone()),
            LockedSource::Git {
                git,
                commit,
                subdir,
                ..
            } => Source::Git(git.clone(), commit.clone(), subdir.clone()),
            LockedSource::Registry { registry, .. } => Source::Registry(registry.to_string()),
            LockedSource::Artifact { path } => Source::Artifact(path.clone()),
            LockedSource::Sysroot { name } => Source::Sysroot(name.clone()),
        };
        Self {
            version: coordinate
                .version()
                .parse()
                .expect("coordinate has a valid version"),
            source,
        }
    }

    pub fn preference(&self, other: &Self) -> std::cmp::Ordering {
        other
            .version
            .cmp_precedence(&self.version)
            .then_with(|| self.version.to_string().cmp(&other.version.to_string()))
            .then_with(|| self.source.cmp(&other.source))
    }
}

impl fmt::Display for Choice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [", self.version)?;
        match &self.source {
            Source::First | Source::Last => f.write_str("any source"),
            Source::Path(path) => write!(f, "path {path}"),
            Source::Git(git, commit, subdir) => write!(f, "git {git}#{commit}:{subdir}"),
            Source::Registry(registry) => write!(f, "registry {registry}"),
            Source::Artifact(path) => write!(f, "artifact {path}"),
            Source::Sysroot(name) => write!(f, "sysroot {name}"),
        }?;
        f.write_str("]")
    }
}
