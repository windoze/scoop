//! Shared dependency-graph validation for cross-Cone profile closures.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir::ValidatedLirTargetSelection;

use super::{
    CrossConeClosureGraphError, DecodedCrossConeClosure, ProfileValidatedCrossConeHirClosure,
};
use crate::{ConeKind, DecodedCrossConeHirFrontSections, DependencyRecord};

pub(crate) trait CrossConeClosureArtifact {
    fn coordinate(&self) -> &ConeCoordinate;
    fn identity(&self) -> ConeIdentity;
    fn kind(&self) -> ConeKind;
    fn target_selection(&self) -> ValidatedLirTargetSelection;
    fn direct_dependencies(&self) -> &[DependencyRecord];
    fn dependency_record(&self) -> DependencyRecord;
}

impl CrossConeClosureArtifact for DecodedCrossConeHirFrontSections<'_> {
    fn coordinate(&self) -> &ConeCoordinate {
        self.coordinate()
    }

    fn identity(&self) -> ConeIdentity {
        self.identity()
    }

    fn kind(&self) -> ConeKind {
        self.kind()
    }

    fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target_selection()
    }

    fn direct_dependencies(&self) -> &[DependencyRecord] {
        self.direct_dependencies()
    }

    fn dependency_record(&self) -> DependencyRecord {
        self.dependency_record()
    }
}

pub(crate) struct ValidatedCrossConeClosureGraph<T> {
    pub(crate) current: ConeIdentity,
    pub(crate) target: ValidatedLirTargetSelection,
    pub(crate) direct: Vec<ConeIdentity>,
    pub(crate) dependency_first: Vec<T>,
    pub(crate) positions: BTreeMap<ConeIdentity, usize>,
    pub(crate) dependency_positions: Vec<Vec<usize>>,
}

pub(super) fn validate_profile_graph(
    closure: DecodedCrossConeClosure<'_>,
) -> Result<ProfileValidatedCrossConeHirClosure<'_>, CrossConeClosureGraphError> {
    let DecodedCrossConeClosure {
        current,
        target,
        direct,
        dependency_first,
        current_artifact,
    } = closure;
    let validated =
        validate_artifact_graph(current, target, direct, dependency_first, current_artifact)?;
    Ok(ProfileValidatedCrossConeHirClosure {
        current: validated.current,
        target: validated.target,
        direct: validated.direct,
        dependency_first: validated.dependency_first,
        positions: validated.positions,
        dependency_positions: validated.dependency_positions,
    })
}

pub(crate) fn validate_artifact_graph<T: CrossConeClosureArtifact>(
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    mut dependency_first: Vec<T>,
    current_artifact: Option<T>,
) -> Result<ValidatedCrossConeClosureGraph<T>, CrossConeClosureGraphError> {
    validate_direct_order(&direct)?;
    if current == ConeIdentity::CORE && current_artifact.is_none() {
        if !direct.is_empty() || !dependency_first.is_empty() {
            return Err(CrossConeClosureGraphError::CoreHasDependencyProviders);
        }
    } else if current != ConeIdentity::CORE && direct.binary_search(&ConeIdentity::CORE).is_err() {
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
        validate_unique_version(&mut versions, artifact.coordinate())?;
    }

    let has_current_artifact = current_artifact.is_some();
    if let Some(artifact) = current_artifact {
        if artifact.identity() != current {
            return Err(
                CrossConeClosureGraphError::CurrentArtifactIdentityMismatch {
                    expected: current,
                    actual: artifact.identity(),
                },
            );
        }
        if artifact.target_selection() != target {
            return Err(CrossConeClosureGraphError::TargetMismatch {
                identity: current,
                expected: target,
                actual: artifact.target_selection(),
            });
        }
        let artifact_direct = artifact
            .direct_dependencies()
            .iter()
            .map(DependencyRecord::identity)
            .collect::<Vec<_>>();
        if artifact_direct != direct {
            return Err(CrossConeClosureGraphError::CurrentDirectSetMismatch {
                expected: direct.clone(),
                actual: artifact_direct,
            });
        }
        validate_unique_version(&mut versions, artifact.coordinate())?;
        positions.insert(current, dependency_first.len());
        dependency_first.push(artifact);
    }

    for identity in &direct {
        if !positions.contains_key(identity) {
            return Err(CrossConeClosureGraphError::MissingDirectArtifact {
                identity: *identity,
            });
        }
    }

    let mut dependency_positions = Vec::with_capacity(dependency_first.len());
    for (position, artifact) in dependency_first.iter().enumerate() {
        let dependent = artifact.identity();
        let mut direct_positions = Vec::with_capacity(artifact.direct_dependencies().len());
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
            direct_positions.push(dependency_position);
        }
        dependency_positions.push(direct_positions);
    }

    let mut reachable = BTreeSet::new();
    let mut pending = if has_current_artifact {
        vec![current]
    } else {
        direct.clone()
    };
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

    Ok(ValidatedCrossConeClosureGraph {
        current,
        target,
        direct,
        dependency_first,
        positions,
        dependency_positions,
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
