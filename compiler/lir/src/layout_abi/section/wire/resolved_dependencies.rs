//! Owned dependency transport shared by complete and staged readers.

use super::*;

/// Identity-resolved semantic records and the original physical transport.
/// Graph replay grants neither a selected machine reference nor publication.
pub struct DependencyResolvedCrossConeLayoutAbiSectionV1 {
    pub(super) exports: LayoutAbiExportConstituentsV1,
    pub(super) semantic: Vec<LayoutAbiDependencyV1>,
    pub(super) physical: crate::DecodedCanonicalExternalShapeLinkImportsV1,
}

impl ExportsResolvedCrossConeLayoutAbiSectionV1 {
    pub fn resolve_dependencies(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<DependencyResolvedCrossConeLayoutAbiSectionV1, LayoutAbiSectionError> {
        dispatch_inventory::validate(&self.exports)?;
        let mut semantic = reserve(self.selected.semantic.len())?;
        for relation in self.selected.semantic {
            semantic.push(relation.resolve(identities)?);
        }
        build::validate_selected_records(self.exports.provider(), &semantic)?;
        Ok(DependencyResolvedCrossConeLayoutAbiSectionV1 {
            exports: self.exports,
            semantic,
            physical: self.selected.physical,
        })
    }
}

impl DependencyResolvedCrossConeLayoutAbiSectionV1 {
    pub const fn exports(&self) -> &LayoutAbiExportConstituentsV1 {
        &self.exports
    }

    pub fn selected_relations(&self) -> &[LayoutAbiDependencyV1] {
        &self.semantic
    }

    /// Uses all exports of exactly the caller's reachable dependency graph.
    /// Committed roots must come from the containing artifact's source replay.
    pub fn replay_dependency_closure(
        &self,
        dependencies: &[&LayoutAbiExportConstituentsV1],
        committed: &[LayoutAbiDependencyV1],
    ) -> Result<(), LayoutAbiSectionError> {
        dependencies::validate_exports(
            self.exports.provider(),
            self.exports.target_profile(),
            dependencies,
        )?;
        build::close_selection(&self.exports, dependencies, committed, Some(&self.semantic))?;
        Ok(())
    }
}
