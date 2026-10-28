use std::collections::{BTreeSet, VecDeque};

use pubgrub::{DefaultStringReporter, DerivationTree, Reporter};

use super::{
    provider::{Domain, Pool},
    *,
};
use crate::PmError;

pub(super) fn failure(
    root: &Key,
    pool: &Pool,
    policy: &ResolvePolicy,
    tree: DerivationTree<Key, Domain, String>,
) -> PmError {
    let involved = tree.packages();
    let mut error = PmError::new(format!(
        "dependency resolution failed:\n{}",
        DefaultStringReporter::report(&tree)
    ));
    error.resolution_conflicts = involved.iter().map(|key| (*key).clone()).collect();
    let mut paths = BTreeMap::from([(root.clone(), root.to_string())]);
    let mut queue = VecDeque::from([root.clone()]);
    let mut seen = BTreeSet::new();
    while let Some(key) = queue.pop_front() {
        if !seen.insert(key.clone()) {
            continue;
        }
        let Some(choices) = pool.get(&key) else {
            continue;
        };
        for candidate in choices.values() {
            for edge in &candidate.dependencies {
                let path = format!("{} -> {}", paths[&key], edge.key);
                paths
                    .entry(edge.key.clone())
                    .or_insert_with(|| path.clone());
                queue.push_back(edge.key.clone());
                if involved.contains(&edge.key) {
                    let version = if edge.versions.is_empty() {
                        "source version".into()
                    } else {
                        edge.versions
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(" and ")
                    };
                    error.message.push_str(&format!(
                        "\n  {path}: {} requires {version} from {}",
                        candidate.coordinate, edge.source
                    ));
                    for origin in &edge.origins {
                        error.message.push_str(&format!(
                            "\n    {} (bytes {}..{})",
                            origin.manifest.display(),
                            origin.span.start,
                            origin.span.end
                        ));
                        if !error.origins.contains(origin) {
                            error.origins.push(origin.clone());
                        }
                    }
                    if let Some(candidates) = pool.get(&edge.key) {
                        for (choice, candidate) in candidates {
                            let mut reasons = Vec::new();
                            if candidate.kind != RequestedConeKind::Library {
                                reasons.push("executable; dependencies must be libraries".into());
                            }
                            for version in &edge.versions {
                                if !version.matches(&choice.version) {
                                    reasons.push(format!("does not satisfy {version}"));
                                }
                            }
                            if !policy.permitted(candidate) {
                                reasons.push("excluded by lock, update, or yank policy".into());
                            }
                            if reasons.is_empty() {
                                reasons.push("other version or source constraints conflict".into());
                            }
                            error.message.push_str(&format!(
                                "\n    candidate {choice}: {}",
                                reasons.join("; ")
                            ));
                        }
                    }
                }
            }
        }
    }
    error
}

impl std::fmt::Display for SourceRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(path) => write!(f, "path {}", path.display()),
            Self::Artifact(path) => write!(f, "artifact {}", path.display()),
            Self::Registry(registry) => write!(f, "registry {registry}"),
            Self::Git { source, .. } => write!(
                f,
                "git {} ({})",
                source.repository,
                reference_key(&source.reference)
            ),
            Self::GitPath { checkout, subdir } => write!(
                f,
                "git {}#{}:{subdir}",
                checkout.repository, checkout.commit
            ),
            Self::ExactArtifact(coordinate) => write!(f, "exact artifact {coordinate}"),
            Self::System => f.write_str("sysroot/artifact search roots"),
            Self::ImplicitCore => f.write_str("shared implicit core"),
        }
    }
}
