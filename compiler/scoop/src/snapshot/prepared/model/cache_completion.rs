use scoop_identity::ConeIdentity;

use super::{PreparedBuildGraph, PreparedGraphNode};
use crate::{CacheCompletionError, CompletedNode, RawCompileCacheEntryV1, validate_cache_entry};

impl PreparedBuildGraph {
    /// Promotes a raw cache pair only after the exact current graph, receipt,
    /// dual artifact views, and both purpose closures agree.
    pub fn complete_cache_hit(
        &mut self,
        identity: ConeIdentity,
        entry: RawCompileCacheEntryV1,
        completed: &[&CompletedNode],
    ) -> Result<CompletedNode, CacheCompletionError> {
        if !matches!(
            self.nodes.get(&identity),
            Some(PreparedGraphNode::ManifestSource(_) | PreparedGraphNode::SingleFile(_))
        ) {
            return Err(CacheCompletionError::NotOrdinarySource(identity));
        }
        let expected_key = self
            .compile_cache_key(identity, completed)
            .map_err(|source| CacheCompletionError::CacheKey(Box::new(source)))?;
        let plan = self.artifact_closure_plan();
        let limits = self.context.limits.artifact_decode();
        let c_bridge_profile = self.context.target.c_bridge_toolchain().profile().clone();
        let validated = validate_cache_entry(
            &plan,
            identity,
            expected_key,
            entry,
            completed,
            self.compiler.fingerprint(),
            limits,
            self.target_selection,
            &c_bridge_profile,
            &mut self.meter,
        )?;
        let snapshot = validated.artifact().snapshot();
        let materialized = self
            .staging
            .materialize_cache_artifact(
                &identity.to_string(),
                snapshot.as_bytes(),
                snapshot.digest(),
            )
            .map_err(CacheCompletionError::Staging)?;
        Ok(validated.into_completed(materialized))
    }
}
