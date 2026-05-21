//! Temporary source-site fact adapter kept until P2-T05 moves these contracts into `HirFacts`.

use crate::hir;

/// Source-site-only facts still waiting for the P2-T05 contract migration.
///
/// Declaration/entity/global/native facts are owned by `scoopc_hir_facts::HirFacts`.
/// This adapter intentionally carries only call-site facts that are not part of
/// P2-T04, so it does not overlap with the HIR declaration fact query surface.
#[derive(Debug, Clone, Default)]
pub(crate) struct SourceSiteMigrationFacts {
    pub(crate) ctor_call_targets: hir::CtorCallSiteIndex,
    pub(crate) continuation_resume_call_sites: hir::ContinuationResumeCallSiteIndex,
    pub(crate) non_pure_continuation_resume_call_sites: hir::NonPureContinuationResumeCallSiteIndex,
}

impl SourceSiteMigrationFacts {
    /// Copy the remaining source-site side tables from HIR until P2-T05 removes this bridge.
    #[cfg(any(test, feature = "llvm"))]
    pub(crate) fn from_hir_side_tables(lowered: &hir::LoweredHir) -> Self {
        Self {
            ctor_call_targets: lowered.ctor_call_sites.clone(),
            continuation_resume_call_sites: lowered.continuation_resume_call_sites.clone(),
            non_pure_continuation_resume_call_sites: lowered
                .non_pure_continuation_resume_call_sites
                .clone(),
        }
    }
}
