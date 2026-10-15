//! Saved dependency transport and typed references.

use super::*;

/// Parsed MIR export data and typed dependency references.
pub struct DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
    pub(super) provider: ConeIdentity,
    pub(super) exports: MirTypeBridgeExportConstituentsV1,
    pub(super) direct: crate::CrossConeMirBridgeSectionV1,
    pub(super) selected: Vec<MirTypeBridgeDependencyV1>,
}

impl DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn exports(&self) -> &MirTypeBridgeExportConstituentsV1 {
        &self.exports
    }

    pub fn selected_relations(&self) -> &[MirTypeBridgeDependencyV1] {
        &self.selected
    }

    pub fn dependency_view<'a>(
        &'a self,
        units: &'a [MirTypeBridgeInitializationUnitV1],
    ) -> MirTypeBridgeDependencyViewV1<'a> {
        view::LocalView {
            provider: self.provider,
            exports: &self.exports,
            units,
            direct: &self.direct,
        }
    }
}
