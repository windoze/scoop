//! Graph and provider-role validation for the M23-5 semantic closure.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir::ValidatedLirTargetSelection;

use crate::{ConeKind, DecodedCrossConeHirFrontSections, DependencyRecord};

/// Untrusted assembly input for the dependency artifacts visible while
/// compiling one current Cone.
pub struct DecodedCrossConeClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<DecodedCrossConeHirFrontSections<'input>>,
}

impl<'input> DecodedCrossConeClosure<'input> {
    pub const fn new(
        current: ConeIdentity,
        target: ValidatedLirTargetSelection,
        direct: Vec<ConeIdentity>,
        dependency_first: Vec<DecodedCrossConeHirFrontSections<'input>>,
    ) -> Self {
        Self {
            current,
            target,
            direct,
            dependency_first,
        }
    }

    /// Replays the graph facts needed by semantic import without trusting
    /// artifact argument order or role labels supplied by the caller.
    pub fn validate_profile_graph(
        self,
    ) -> Result<ProfileValidatedCrossConeHirClosure<'input>, CrossConeClosureGraphError> {
        validate_profile_graph(self)
    }
}

/// Directness is a closed role assigned only after the whole dependency
/// graph has been checked. A support provider cannot be promoted by a bool.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CrossConeProviderRole {
    Direct,
    Support,
}

/// Cross-Cone HIR fronts whose exact profile and complete dependency graph
/// agree. Identity, surface, route, and bridge obligations remain unproven.
pub struct ProfileValidatedCrossConeHirClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<DecodedCrossConeHirFrontSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
}

impl ProfileValidatedCrossConeHirClosure<'_> {
    pub const fn current(&self) -> ConeIdentity {
        self.current
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub fn direct_providers(&self) -> &[ConeIdentity] {
        &self.direct
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &DecodedCrossConeHirFrontSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&DecodedCrossConeHirFrontSections<'_>> {
        self.positions
            .get(&identity)
            .map(|position| &self.dependency_first[*position])
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
        self.positions.get(&identity).map(|_| {
            if self.direct.binary_search(&identity).is_ok() {
                CrossConeProviderRole::Direct
            } else {
                CrossConeProviderRole::Support
            }
        })
    }
}

fn validate_profile_graph(
    closure: DecodedCrossConeClosure<'_>,
) -> Result<ProfileValidatedCrossConeHirClosure<'_>, CrossConeClosureGraphError> {
    let DecodedCrossConeClosure {
        current,
        target,
        direct,
        dependency_first,
    } = closure;

    validate_direct_order(&direct)?;
    if current == ConeIdentity::CORE {
        if !direct.is_empty() || !dependency_first.is_empty() {
            return Err(CrossConeClosureGraphError::CoreHasDependencyProviders);
        }
    } else if direct.binary_search(&ConeIdentity::CORE).is_err() {
        return Err(CrossConeClosureGraphError::MissingTrustedCore);
    }

    let mut positions = BTreeMap::new();
    let mut versions = BTreeMap::<(String, String), ConeCoordinate>::new();
    for (position, artifact) in dependency_first.iter().enumerate() {
        let identity = artifact.identity();
        if identity == current {
            return Err(CrossConeClosureGraphError::CurrentArtifactPresent { current });
        }
        if positions.insert(identity, position).is_some() {
            return Err(CrossConeClosureGraphError::DuplicateArtifact { identity });
        }
        if artifact.kind() != ConeKind::Library {
            return Err(CrossConeClosureGraphError::InvalidProviderKind {
                identity,
                actual: artifact.kind(),
            });
        }
        if artifact.target_selection() != target {
            return Err(CrossConeClosureGraphError::TargetMismatch {
                identity,
                expected: target,
                actual: artifact.target_selection(),
            });
        }
        validate_unique_version(&mut versions, artifact.coordinate(), identity)?;
    }

    for identity in &direct {
        if !positions.contains_key(identity) {
            return Err(CrossConeClosureGraphError::MissingDirectArtifact {
                identity: *identity,
            });
        }
    }

    for (position, artifact) in dependency_first.iter().enumerate() {
        let dependent = artifact.identity();
        for recorded in artifact.direct_dependencies() {
            let dependency = recorded.identity();
            let Some(dependency_position) = positions.get(&dependency).copied() else {
                return Err(CrossConeClosureGraphError::MissingDependencyArtifact {
                    dependent,
                    dependency,
                });
            };
            if dependency_position >= position {
                return Err(CrossConeClosureGraphError::InvalidDependencyFirstOrder {
                    dependent,
                    dependency,
                });
            }
            let actual = dependency_first[dependency_position].dependency_record();
            if recorded != &actual {
                return Err(CrossConeClosureGraphError::StaleDependency {
                    dependent,
                    dependency,
                    recorded: Box::new(recorded.clone()),
                    actual: Box::new(actual),
                });
            }
        }
    }

    let mut reachable = BTreeSet::new();
    let mut pending = direct.clone();
    while let Some(identity) = pending.pop() {
        if !reachable.insert(identity) {
            continue;
        }
        let artifact = &dependency_first[positions[&identity]];
        pending.extend(
            artifact
                .direct_dependencies()
                .iter()
                .rev()
                .map(DependencyRecord::identity),
        );
    }
    if let Some(artifact) = dependency_first
        .iter()
        .find(|artifact| !reachable.contains(&artifact.identity()))
    {
        return Err(CrossConeClosureGraphError::UnreachableSupport {
            identity: artifact.identity(),
        });
    }

    Ok(ProfileValidatedCrossConeHirClosure {
        current,
        target,
        direct,
        dependency_first,
        positions,
    })
}

fn validate_direct_order(direct: &[ConeIdentity]) -> Result<(), CrossConeClosureGraphError> {
    for (index, pair) in direct.windows(2).enumerate() {
        if pair[0] >= pair[1] {
            return Err(CrossConeClosureGraphError::NonCanonicalDirectProviders {
                index: index + 1,
                previous: pair[0],
                actual: pair[1],
            });
        }
    }
    Ok(())
}

fn validate_unique_version(
    versions: &mut BTreeMap<(String, String), ConeCoordinate>,
    coordinate: &ConeCoordinate,
    _identity: ConeIdentity,
) -> Result<(), CrossConeClosureGraphError> {
    let key = (coordinate.group().to_owned(), coordinate.name().to_owned());
    if let Some(first) = versions.get(&key)
        && first.version() != coordinate.version()
    {
        return Err(CrossConeClosureGraphError::MultipleVersions {
            first: Box::new(first.clone()),
            second: Box::new(coordinate.clone()),
        });
    }
    versions.insert(key, coordinate.clone());
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeClosureGraphError {
    CoreHasDependencyProviders,
    MissingTrustedCore,
    NonCanonicalDirectProviders {
        index: usize,
        previous: ConeIdentity,
        actual: ConeIdentity,
    },
    CurrentArtifactPresent {
        current: ConeIdentity,
    },
    DuplicateArtifact {
        identity: ConeIdentity,
    },
    InvalidProviderKind {
        identity: ConeIdentity,
        actual: ConeKind,
    },
    TargetMismatch {
        identity: ConeIdentity,
        expected: ValidatedLirTargetSelection,
        actual: ValidatedLirTargetSelection,
    },
    MultipleVersions {
        first: Box<ConeCoordinate>,
        second: Box<ConeCoordinate>,
    },
    MissingDirectArtifact {
        identity: ConeIdentity,
    },
    MissingDependencyArtifact {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    InvalidDependencyFirstOrder {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    StaleDependency {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
        recorded: Box<DependencyRecord>,
        actual: Box<DependencyRecord>,
    },
    UnreachableSupport {
        identity: ConeIdentity,
    },
}

impl fmt::Display for CrossConeClosureGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid cross-Cone semantic closure graph: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeClosureGraphError {}

#[cfg(test)]
mod tests;
