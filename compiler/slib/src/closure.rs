//! Explicit artifact-closure validation (DESIGN sections 1.4 and 5.5).
//!
//! `scoopc` receives a closed set of upstream artifacts (trusted core,
//! direct dependencies, transitive support) and re-validates it against
//! the artifacts' own typed identities before parsing any source. This
//! module owns that check: no locator resolution, no version selection,
//! no rebuilding — the caller's set either is a consistent closure or it
//! is rejected.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;

use scoop_identity::{ConeCoordinate, ConeIdentity};

use crate::artifact::ValidatedGraphArtifact;

/// One closure-level inconsistency, reported with the offending
/// coordinate(s). Input order never influences which errors surface:
/// identities are processed in digest order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosureError {
    /// The trusted core slot does not carry the reserved coordinate.
    NotReservedCore {
        actual: Box<ConeCoordinate>,
    },
    /// The core Cone declares dependencies of its own.
    CoreHasDependencies(usize),
    /// One identity was provided more than once.
    DuplicateIdentity {
        identity: ConeIdentity,
    },
    /// One `group:name` appears with multiple versions.
    MultipleVersions {
        group: String,
        name: String,
        first: String,
        second: String,
    },
    /// An executable Cone appears as a dependency.
    ExecutableDependency {
        coordinate: Box<ConeCoordinate>,
    },
    /// An artifact is present but unreachable from the direct edges.
    UnreachableArtifact {
        coordinate: Box<ConeCoordinate>,
    },
    /// An artifact appears in both the direct and support sets.
    MisclassifiedArtifact {
        coordinate: Box<ConeCoordinate>,
    },
    /// A dependency edge in some artifact points outside the closure.
    DanglingEdge {
        from: Box<ConeCoordinate>,
        missing: ConeIdentity,
    },
    Cycle {
        path: Vec<Box<ConeCoordinate>>,
    },
    /// Artifacts were built against different target profiles.
    TargetMismatch {
        first: Box<ConeCoordinate>,
        second: Box<ConeCoordinate>,
    },
    /// Artifacts disagree on schema/ABI versions.
    SchemaMismatch {
        first: Box<ConeCoordinate>,
        second: Box<ConeCoordinate>,
        field: &'static str,
    },
}

impl fmt::Display for ClosureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClosureError::NotReservedCore { actual } => write!(
                f,
                "the trusted core slot must carry scoop:scoop.core:0.1.0, found {actual}"
            ),
            ClosureError::CoreHasDependencies(count) => write!(
                f,
                "the core Cone declares {count} dependencies; the bootstrap root depends on nothing"
            ),
            ClosureError::DuplicateIdentity { identity } => {
                write!(f, "artifact {identity} was provided more than once")
            }
            ClosureError::MultipleVersions {
                group,
                name,
                first,
                second,
            } => write!(
                f,
                "Cone {group}:{name} appears with versions {first} and {second}"
            ),
            ClosureError::ExecutableDependency { coordinate } => {
                write!(f, "executable Cone {coordinate} cannot be a dependency")
            }
            ClosureError::UnreachableArtifact { coordinate } => write!(
                f,
                "artifact {coordinate} is not reachable from the direct dependencies"
            ),
            ClosureError::MisclassifiedArtifact { coordinate } => write!(
                f,
                "artifact {coordinate} was passed as both direct and support input"
            ),
            ClosureError::DanglingEdge { from, missing } => write!(
                f,
                "artifact {from} depends on {missing}, which is not part of the input set"
            ),
            ClosureError::Cycle { path } => write!(
                f,
                "dependency cycle: {}",
                path.iter()
                    .map(|coordinate| coordinate.display())
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ),
            ClosureError::TargetMismatch { first, second } => write!(
                f,
                "artifacts {first} and {second} were built for different target profiles"
            ),
            ClosureError::SchemaMismatch {
                first,
                second,
                field,
            } => write!(f, "artifacts {first} and {second} disagree on {field}"),
        }
    }
}

impl std::error::Error for ClosureError {}

/// A validated closure of upstream artifacts plus its purpose marker.
pub struct ValidatedArtifactClosure<P> {
    marker: PhantomData<P>,
    core: ValidatedGraphArtifact,
    direct: Vec<ValidatedGraphArtifact>,
    support: Vec<ValidatedGraphArtifact>,
}

impl<P> fmt::Debug for ValidatedArtifactClosure<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidatedArtifactClosure")
            .field("core", self.core.coordinate())
            .field("direct", &self.direct.len())
            .field("support", &self.support.len())
            .finish()
    }
}

impl<P> ValidatedArtifactClosure<P> {
    pub fn core(&self) -> &ValidatedGraphArtifact {
        &self.core
    }

    pub fn direct(&self) -> &[ValidatedGraphArtifact] {
        &self.direct
    }

    pub fn support(&self) -> &[ValidatedGraphArtifact] {
        &self.support
    }

    /// All artifacts in canonical (identity digest) order, core first.
    pub fn all_artifacts(&self) -> Vec<&ValidatedGraphArtifact> {
        let mut all = vec![&self.core];
        all.extend(self.direct.iter());
        all.extend(self.support.iter());
        all
    }
}

/// Validates an explicit core/direct/support artifact set into a
/// purpose-marked closure. Argument order is irrelevant: the set is
/// keyed by `ConeIdentity` before any check.
pub fn validate_explicit_closure<P>(
    core: ValidatedGraphArtifact,
    mut direct: Vec<ValidatedGraphArtifact>,
    mut support: Vec<ValidatedGraphArtifact>,
) -> Result<ValidatedArtifactClosure<P>, ClosureError> {
    if !core.coordinate().is_reserved_core() {
        return Err(ClosureError::NotReservedCore {
            actual: Box::new(core.coordinate().clone()),
        });
    }
    if !core.dependencies().is_empty() {
        return Err(ClosureError::CoreHasDependencies(core.dependencies().len()));
    }
    direct.sort_by_key(ValidatedGraphArtifact::cone_identity);
    support.sort_by_key(ValidatedGraphArtifact::cone_identity);

    // Duplicate identities across the whole input.
    let mut identities: BTreeSet<ConeIdentity> = BTreeSet::new();
    for artifact in direct.iter().chain(support.iter()) {
        if !identities.insert(artifact.cone_identity()) {
            return Err(ClosureError::DuplicateIdentity {
                identity: artifact.cone_identity(),
            });
        }
    }
    // Direct/support overlap.
    let direct_ids: BTreeSet<ConeIdentity> = direct
        .iter()
        .map(ValidatedGraphArtifact::cone_identity)
        .collect();
    for artifact in &support {
        if direct_ids.contains(&artifact.cone_identity()) {
            return Err(ClosureError::MisclassifiedArtifact {
                coordinate: Box::new(artifact.coordinate().clone()),
            });
        }
    }

    // One version per group:name across the entire closure.
    let mut versions: BTreeMap<(String, String), String> = BTreeMap::new();
    for artifact in std::iter::once(&core)
        .chain(direct.iter())
        .chain(support.iter())
    {
        let coordinate = artifact.coordinate();
        let key = (coordinate.group().to_owned(), coordinate.name().to_owned());
        match versions.entry(key) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(coordinate.version().as_str().to_owned());
            }
            std::collections::btree_map::Entry::Occupied(slot) => {
                if slot.get() != coordinate.version().as_str() {
                    return Err(ClosureError::MultipleVersions {
                        group: artifact.coordinate().group().to_owned(),
                        name: artifact.coordinate().name().to_owned(),
                        first: slot.get().clone(),
                        second: coordinate.version().as_str().to_owned(),
                    });
                }
            }
        }
    }

    // Executables cannot be dependencies.
    for artifact in direct.iter().chain(support.iter()) {
        if artifact.kind() == scoop_manifest::ConeKind::Executable {
            return Err(ClosureError::ExecutableDependency {
                coordinate: Box::new(artifact.coordinate().clone()),
            });
        }
    }

    // Cross-artifact compatibility: target and schema/ABI fields.
    let reference = core.manifest();
    for artifact in direct.iter().chain(support.iter()) {
        let candidate = artifact.manifest();
        if candidate.target_profile != reference.target_profile
            || candidate.target_profile_fingerprint != reference.target_profile_fingerprint
        {
            return Err(ClosureError::TargetMismatch {
                first: Box::new(core.coordinate().clone()),
                second: Box::new(artifact.coordinate().clone()),
            });
        }
        let schema_fields = [
            (
                "container version",
                reference.container_version,
                candidate.container_version,
            ),
            (
                "HIR wire schema",
                reference.hir_wire_schema,
                candidate.hir_wire_schema,
            ),
            (
                "MIR wire schema",
                reference.mir_wire_schema,
                candidate.mir_wire_schema,
            ),
            (
                "LIR wire schema",
                reference.lir_wire_schema,
                candidate.lir_wire_schema,
            ),
            (
                "language ABI",
                reference.language_abi,
                candidate.language_abi,
            ),
            (
                "identity schema version",
                reference.identity_schema_version,
                candidate.identity_schema_version,
            ),
        ];
        for (field, expected, actual) in schema_fields {
            if expected != actual {
                return Err(ClosureError::SchemaMismatch {
                    first: Box::new(core.coordinate().clone()),
                    second: Box::new(artifact.coordinate().clone()),
                    field,
                });
            }
        }
        if candidate.runtime_abi != reference.runtime_abi {
            return Err(ClosureError::SchemaMismatch {
                first: Box::new(core.coordinate().clone()),
                second: Box::new(artifact.coordinate().clone()),
                field: "runtime ABI",
            });
        }
    }

    // Reachability: every artifact's dependency edges must resolve inside
    // the closure, and the support set must be exactly the transitive
    // dependencies beyond the direct set and core.
    let mut by_identity: BTreeMap<ConeIdentity, &ValidatedGraphArtifact> = BTreeMap::new();
    by_identity.insert(core.cone_identity(), &core);
    for artifact in &direct {
        by_identity.insert(artifact.cone_identity(), artifact);
    }
    for artifact in &support {
        by_identity.insert(artifact.cone_identity(), artifact);
    }
    let mut reachable: BTreeSet<ConeIdentity> = BTreeSet::new();
    let mut queue: Vec<ConeIdentity> = direct
        .iter()
        .map(ValidatedGraphArtifact::cone_identity)
        .collect();
    while let Some(identity) = queue.pop() {
        if !reachable.insert(identity) {
            continue;
        }
        let artifact = by_identity[&identity];
        for dependency in artifact.dependencies() {
            let target = dependency.cone_identity;
            if !by_identity.contains_key(&target) {
                return Err(ClosureError::DanglingEdge {
                    from: Box::new(artifact.coordinate().clone()),
                    missing: target,
                });
            }
            if !reachable.contains(&target) {
                queue.push(target);
            }
        }
    }
    // Everything except core and the direct set must be reachable.
    for artifact in &support {
        if !reachable.contains(&artifact.cone_identity()) {
            return Err(ClosureError::UnreachableArtifact {
                coordinate: Box::new(artifact.coordinate().clone()),
            });
        }
    }

    // Cycle detection (including self-loops) over the resolved graph.
    // Inputs are valid `.slib`s so cycles indicate corrupted metadata,
    // but the check is cheap and the failure must be stable.
    detect_cycle(&core, &direct, &support)?;

    Ok(ValidatedArtifactClosure {
        marker: PhantomData,
        core,
        direct,
        support,
    })
}

fn detect_cycle(
    core: &ValidatedGraphArtifact,
    direct: &[ValidatedGraphArtifact],
    support: &[ValidatedGraphArtifact],
) -> Result<(), ClosureError> {
    // Tarjan-free iterative DFS with explicit colors keyed by identity.
    let mut ids: Vec<ConeIdentity> = Vec::with_capacity(direct.len() + support.len() + 1);
    ids.push(core.cone_identity());
    ids.extend(direct.iter().map(ValidatedGraphArtifact::cone_identity));
    ids.extend(support.iter().map(ValidatedGraphArtifact::cone_identity));
    let mut index_of: BTreeMap<ConeIdentity, usize> = BTreeMap::new();
    for (index, identity) in ids.iter().enumerate() {
        index_of.insert(*identity, index);
    }
    // Adjacency: artifact -> dependency identities within the closure.
    let adjacency: Vec<Vec<usize>> = std::iter::once(core)
        .chain(direct.iter())
        .chain(support.iter())
        .map(|artifact| {
            artifact
                .dependencies()
                .iter()
                .filter_map(|dependency| index_of.get(&dependency.cone_identity).copied())
                .collect()
        })
        .collect();

    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }
    let mut colors = vec![Color::White; ids.len()];
    // Iterative DFS preserving the path for the diagnostic.
    for start in 0..ids.len() {
        if colors[start] != Color::White {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        let mut path: Vec<usize> = vec![start];
        colors[start] = Color::Gray;
        while let Some((node, next_edge)) = stack.last_mut() {
            if *next_edge >= adjacency[*node].len() {
                colors[*node] = Color::Black;
                path.pop();
                stack.pop();
                continue;
            }
            let target = adjacency[*node][*next_edge];
            *next_edge += 1;
            match colors[target] {
                Color::White => {
                    colors[target] = Color::Gray;
                    path.push(target);
                    stack.push((target, 0));
                }
                Color::Gray => {
                    let cycle_start = path.iter().position(|n| *n == target).unwrap_or(0);
                    let path_coordinates: Vec<Box<ConeCoordinate>> = path[cycle_start..]
                        .iter()
                        .map(|n| ids[*n])
                        .map(|identity| coordinate_of(core, direct, support, identity))
                        .map(Box::new)
                        .collect();
                    return Err(ClosureError::Cycle {
                        path: path_coordinates,
                    });
                }
                Color::Black => {}
            }
        }
    }
    Ok(())
}

fn coordinate_of<'a>(
    core: &'a ValidatedGraphArtifact,
    direct: &'a [ValidatedGraphArtifact],
    support: &'a [ValidatedGraphArtifact],
    identity: ConeIdentity,
) -> ConeCoordinate {
    std::iter::once(core)
        .chain(direct.iter())
        .chain(support.iter())
        .find(|artifact| artifact.cone_identity() == identity)
        .map(ValidatedGraphArtifact::coordinate)
        .cloned()
        .expect("closure membership was validated before cycle detection")
}
