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
    pub fn resolve_dependencies<E>(
        self,
        identities: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<DependencyResolvedCrossConeLayoutAbiSectionV1, LayoutAbiSectionError<E>> {
        dispatch_inventory::validate(&self.exports, meter)?;
        let mut semantic = reserve(self.selected.semantic.len(), meter)?;
        for relation in self.selected.semantic {
            semantic.push(relation.resolve(identities, meter)?);
        }
        build::validate_selected_records(self.exports.provider(), &semantic, meter)?;
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
    pub fn replay_dependency_closure<E>(
        &self,
        dependencies: &[&LayoutAbiExportConstituentsV1],
        committed: &[LayoutAbiDependencyV1],
        meter: &mut BudgetMeter,
    ) -> Result<(), LayoutAbiSectionError<E>> {
        dependencies::validate_exports(
            self.exports.provider(),
            self.exports.target_profile(),
            dependencies,
            meter,
        )?;
        build::close_selection(
            &self.exports,
            dependencies,
            committed,
            Some(&self.semantic),
            meter,
        )?;
        Ok(())
    }
}
