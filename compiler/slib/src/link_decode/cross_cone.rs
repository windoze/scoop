//! Link data shares the complete semantic records read from the same archive.

use crate::{PhysicalImportsReplayedCrossConeLayoutSections, ReplayedLayoutLinkSymbolUsesV1};
use std::rc::Rc;

pub struct ValidatedCrossConeStrongLinkArtifact {
    artifact: Rc<PhysicalImportsReplayedCrossConeLayoutSections>,
    symbols: ReplayedLayoutLinkSymbolUsesV1,
}

impl ValidatedCrossConeStrongLinkArtifact {
    pub(crate) fn new(
        artifact: Rc<PhysicalImportsReplayedCrossConeLayoutSections>,
        symbols: ReplayedLayoutLinkSymbolUsesV1,
    ) -> Self {
        Self { artifact, symbols }
    }
    pub fn coordinate(&self) -> &scoop_identity::ConeCoordinate {
        self.artifact.coordinate()
    }
    pub fn identity(&self) -> scoop_identity::ConeIdentity {
        self.artifact.identity()
    }
    pub fn kind(&self) -> crate::ConeKind {
        self.artifact.manifest().cone().kind()
    }
    pub fn source_form(&self) -> crate::ConeSourceForm {
        self.artifact.manifest().cone().source_form()
    }
    pub fn target_selection(&self) -> scoop_lir::ValidatedLirTargetSelection {
        self.artifact.metadata().target_selection()
    }
    pub fn compatibility(&self) -> &crate::CompatibilityRecord {
        self.artifact.manifest().compatibility()
    }
    pub fn direct_dependencies(&self) -> &[crate::DependencyRecord] {
        self.artifact.manifest().direct_dependencies()
    }
    pub fn artifact_fingerprint(&self) -> crate::ArtifactFingerprint {
        self.artifact.manifest().artifact_fingerprint()
    }
    pub fn semantic_fingerprints(&self) -> crate::SemanticFingerprintRecord {
        self.artifact.manifest().semantic_fingerprints()
    }
    pub fn identity_count(&self) -> usize {
        self.artifact.identity_graph().identity_count()
    }
    pub fn hir_foundation(&self) -> &scoop_hir::OdrFreeHirFoundation {
        self.artifact.hir_foundation()
    }
    pub fn mir_foundation(&self) -> &scoop_mir::OdrFreeMirFoundation {
        self.artifact.mir_foundation()
    }
    pub fn lir_foundation(&self) -> &scoop_lir::ConeLirFoundation {
        self.artifact.lir_foundation()
    }
    pub fn strong_production(&self) -> &scoop_lir::ConeProductionSectionV2 {
        self.artifact.lir_strong_production()
    }
    pub fn defined_symbols(&self) -> &crate::CanonicalDefinedLinkSymbolOwnerSetV1 {
        self.symbols.defined_symbols()
    }
    pub fn symbol_uses(&self) -> &ReplayedLayoutLinkSymbolUsesV1 {
        &self.symbols
    }
}
