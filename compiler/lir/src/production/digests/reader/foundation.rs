//! Resolve local digest owners through existing typed foundation records.

use scoop_identity::*;
use scoop_wire::WireError;

use super::*;

impl DecodedDigestFinalizationPlanV1 {
    /// Resolves the candidate graph, without claiming canonical role coverage.
    /// The complete graph is replayed after its registration relations pass.
    pub fn resolve_foundation(
        self,
        foundation: &crate::ConeLirFoundation,
        dependencies: &crate::StrongTypeReferenceDefinitionsV2,
    ) -> Result<DigestFinalizationPlanV1, DigestPlanReplayError> {
        self.validate_resolved(&mut Foundation(foundation, dependencies), foundation)
            .map_err(DigestPlanReplayError::Validation)
    }
}

struct Foundation<'a>(
    &'a crate::ConeLirFoundation,
    &'a crate::StrongTypeReferenceDefinitionsV2,
);
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
impl PersistentIdResolver<PersistentLayoutId> for Foundation<'_> {
    type Error = IdentityReferenceError;
    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<PersistentLayoutId>,
    ) -> Result<PersistentLayoutId, Self::Error> {
        self.0
            .layouts()
            .iter()
            .map(|record| record.id())
            .chain(
                self.1
                    .layouts()
                    .iter()
                    .flat_map(|table| table.records())
                    .map(|record| record.identity().layout()),
            )
            .find(|id| id.as_array() == decoded.as_array())
            .ok_or(IdentityReferenceError::Missing {
                kind: "PersistentLayoutId",
                id: *decoded.as_array(),
            })
    }
}
impl PersistentIdResolver<PersistentScanId> for Foundation<'_> {
    type Error = IdentityReferenceError;
    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<PersistentScanId>,
    ) -> Result<PersistentScanId, Self::Error> {
        self.0
            .scans()
            .iter()
            .map(|record| record.id())
            .chain(
                self.1
                    .layouts()
                    .iter()
                    .flat_map(|table| table.records())
                    .map(|record| record.scan()),
            )
            .find(|id| id.as_array() == decoded.as_array())
            .ok_or(IdentityReferenceError::Missing {
                kind: "PersistentScanId",
                id: *decoded.as_array(),
            })
    }
}
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
impl PersistentIdResolver<OdrMemberId> for Foundation<'_> {
    type Error = IdentityReferenceError;
    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<OdrMemberId>,
    ) -> Result<OdrMemberId, Self::Error> {
        self.0
            .definition_plans()
            .iter()
            .find_map(|record| match record.key().owner() {
                ObjectDefinitionPlanOwner::Odr { member }
                    if member.as_array() == decoded.as_array() =>
                {
                    Some(member)
                }
                _ => None,
            })
            .ok_or(IdentityReferenceError::Missing {
                kind: "OdrMemberId",
                id: *decoded.as_array(),
            })
    }
}

#[derive(Debug)]
pub enum DigestPlanReplayError {
    Validation(DigestPlanValidationError),
    Encoding,
    Resource(WireError),
}
impl From<WireError> for DigestPlanReplayError {
    fn from(source: WireError) -> Self {
        Self::Resource(source)
    }
}
impl std::fmt::Display for DigestPlanReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid foundation digest graph: {self:?}")
    }
}
impl std::error::Error for DigestPlanReplayError {}
