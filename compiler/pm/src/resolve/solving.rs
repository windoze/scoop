use super::*;
use crate::{lockfile::LockedSource, resolver::Candidate};

pub(super) fn solve(
    root: Candidate,
    policy: &mut ResolvePolicy,
    catalog: &mut Catalog<'_>,
) -> PmResult<Solution> {
    if let Some(solution) = resolver::try_locked(&root, policy, |edge| catalog.candidates(edge))? {
        return Ok(solution);
    }
    loop {
        match resolver::solve(root.clone(), policy, |edge| catalog.candidates(edge)) {
            Ok(solution) => return Ok(solution),
            Err(error) => {
                // Old choices and cached indexes are preferences, not a reason
                // to declare the current graph impossible without trying fresh metadata.
                let refresh: Vec<_> = error
                    .resolution_conflicts
                    .iter()
                    .filter(|key| {
                        !policy.strict
                            && !policy.updating(key)
                            && (catalog.can_refresh_index(key)
                                || policy.locked.get(*key).is_some_and(|node| {
                                    matches!(node.source, LockedSource::Git { .. })
                                }))
                            && !policy
                                .precise
                                .as_ref()
                                .is_some_and(|(precise, _)| precise == *key)
                    })
                    .cloned()
                    .collect();
                if refresh.is_empty() {
                    return Err(error);
                }
                policy.update.get_or_insert_with(Vec::new).extend(refresh);
                catalog.update_policy(policy);
            }
        }
    }
}
