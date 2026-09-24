//! Resolve local digest owners through existing typed foundation records.

use scoop_identity::*;
use scoop_wire::{BudgetMeter, WireError, WirePath, encoded_length};

use super::*;

impl DecodedStrongDigestFinalizationPlanV1 {
    /// Resolves the candidate graph, without claiming canonical role coverage.
    /// The complete graph is replayed after its registration relations pass.
    pub fn resolve_foundation(
        self,
        foundation: &crate::OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<StrongDigestFinalizationPlanV1, StrongDigestPlanReplayError> {
        let path = WirePath::root();
        let nodes = self.nodes.len() as u64;
        meter.check_table_entries(nodes, &path)?;
        meter.charge_work(nodes, &path)?;
        let mut edges = 0_u64;
        let mut patches = 0_u64;
        for node in &self.nodes {
            edges = edges.saturating_add(node.direct_inputs.len() as u64);
            patches = patches.saturating_add(node.patch_intents.len() as u64);
        }
        meter.charge_nodes(nodes, &path)?;
        meter.charge_edges(edges.saturating_add(patches), &path)?;
        let entries = nodes.saturating_add(edges).saturating_add(patches);
        meter.charge_collection_slots(entries.saturating_mul(8), &path)?;
        meter.charge_owned_bytes(entries.saturating_mul(1024), &path)?;
        crate::production::digests::budget::charge_resolution(&self, foundation, meter)?;
        let length = encoded_length(&self).map_err(|_| StrongDigestPlanReplayError::Encoding)?;
        meter.charge_sha256(length.saturating_mul(4), &path)?;
        meter.charge_stable_kahn(nodes, edges, &path)?;
        self.validate_resolved(&mut Foundation(foundation), foundation)
            .map_err(StrongDigestPlanReplayError::Validation)
    }
}

struct Foundation<'a>(&'a crate::OdrFreeLirFoundation);
macro_rules! resolve_record {
    ($id:ty, $records:ident) => {
        impl PersistentIdResolver<$id> for Foundation<'_> {
            type Error = IdentityReferenceError;
            fn resolve(&mut self, decoded: DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                self.0
                    .$records()
                    .iter()
                    .map(|record| record.id())
                    .find(|id| id.as_array() == decoded.as_array())
                    .ok_or(IdentityReferenceError::Missing {
                        kind: stringify!($id),
                        id: *decoded.as_array(),
                    })
            }
        }
    };
}
resolve_record!(PersistentCallableBodyId, callable_bodies);
resolve_record!(PersistentLayoutId, layouts);
resolve_record!(PersistentScanId, scans);
resolve_record!(ObjectDefinitionAtomId, definition_atoms);
resolve_record!(PersistentSafepointSiteId, safepoint_sites);
resolve_record!(ObjectDefinitionPlanId, definition_plans);

impl PersistentIdResolver<ConeIdentity> for Foundation<'_> {
    type Error = IdentityReferenceError;
    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        let producer = self.0.producer();
        (producer.as_array() == decoded.as_array())
            .then_some(producer)
            .ok_or(IdentityReferenceError::Missing {
                kind: "ConeIdentity",
                id: *decoded.as_array(),
            })
    }
}
impl PersistentIdResolver<OdrGroupId> for Foundation<'_> {
    type Error = IdentityReferenceError;
    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<OdrGroupId>,
    ) -> Result<OdrGroupId, Self::Error> {
        Err(IdentityReferenceError::Missing {
            kind: "OdrGroupId",
            id: *decoded.as_array(),
        })
    }
}

#[derive(Debug)]
pub enum StrongDigestPlanReplayError {
    Validation(StrongDigestPlanValidationError),
    Encoding,
    Resource(WireError),
}
impl From<WireError> for StrongDigestPlanReplayError {
    fn from(source: WireError) -> Self {
        Self::Resource(source)
    }
}
impl std::fmt::Display for StrongDigestPlanReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid foundation digest graph: {self:?}")
    }
}
impl std::error::Error for StrongDigestPlanReplayError {}
