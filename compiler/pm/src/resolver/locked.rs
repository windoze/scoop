use std::collections::VecDeque;

use super::*;

/// Match current declarations to the recorded choices without running PubGrub.
/// A valid lock never asks for newer versions or refreshes a Git reference.
pub fn try_locked(
    root: &Candidate,
    policy: &ResolvePolicy,
    mut load: impl FnMut(&Requirement) -> PmResult<Vec<Candidate>>,
) -> PmResult<Option<Solution>> {
    if policy.update.is_some() || !policy.is_locked(root) {
        return Ok(None);
    }
    let mut selected = BTreeMap::from([(root.key(), root.clone())]);
    let mut pending = VecDeque::from(root.dependencies.clone());
    let mut fallback = VecDeque::new();
    loop {
        let (edge, deferred) = if let Some(edge) = pending.pop_front() {
            (edge, false)
        } else if let Some(edge) = fallback.pop_front() {
            (edge, true)
        } else {
            break;
        };
        let implicit = matches!(
            edge.source,
            SourceRequest::ImplicitCore | SourceRequest::ExactArtifact(_)
        );
        if implicit && !deferred {
            fallback.push_back(edge);
            continue;
        }
        let candidates = if implicit && let Some(candidate) = selected.get(&edge.key) {
            vec![candidate.clone()]
        } else {
            load(&edge)?
        };
        let candidate = candidates.into_iter().find(|candidate| {
            candidate.key() == edge.key
                && candidate.kind == RequestedConeKind::Library
                && policy.is_locked(candidate)
                && edge
                    .versions
                    .iter()
                    .all(|version| version.matches(&candidate.choice().version))
        });
        let Some(candidate) = candidate else {
            return Ok(None);
        };
        if let Some(existing) = selected.get(&edge.key) {
            if existing.choice() != candidate.choice() {
                return Ok(None);
            }
        } else {
            pending.extend(candidate.dependencies.clone());
            selected.insert(edge.key, candidate);
        }
    }
    Ok(Some(Solution {
        root: root.key(),
        packages: selected,
    }))
}
