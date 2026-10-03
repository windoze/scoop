//! Complete physical contracts resolved against actual dependency records.

use super::*;
use crate::{CanonicalExternalShapeLinkImportsV1, ShapeLinkProviderV1};

/// Owns the exports, selected relations and resolved physical contracts.
pub struct PhysicalImportsReplayedLayoutAbiSectionV1 {
    exports: LayoutAbiExportConstituentsV1,
    semantic: Vec<LayoutAbiDependencyV1>,
    physical: CanonicalExternalShapeLinkImportsV1,
}

impl DependencyResolvedCrossConeLayoutAbiSectionV1 {
    pub fn replay_physical_imports<'a>(
        self,
        dependencies: &[ShapeLinkProviderV1<'a>],

        identities: &mut ValidatedIdentityGraph,
    ) -> Result<PhysicalImportsReplayedLayoutAbiSectionV1, LayoutAbiSectionError> {
        let path = WirePath::root();
        let mut providers = std::collections::HashSet::new();

        for dependency in dependencies {
            let provider = dependency.provider();

            if provider == self.exports.provider() || providers.contains(&provider) {
                return Err(LayoutAbiSectionError::DuplicateProvider(provider));
            }
            if dependency.target_profile() != self.exports.target_profile() {
                return Err(LayoutAbiSectionError::DependencyTarget { provider });
            }
            scoop_wire::allocation::try_reserve_set(&mut providers, 1, &path)?;
            providers.insert(provider);
        }
        let physical = self
            .physical
            .replay(self.exports.provider(), dependencies, identities)?;
        for import in physical.records() {
            let terminal = dependencies
                .iter()
                .find(|view| view.provider() == import.provider())
                .ok_or(LayoutAbiSectionError::MissingPhysicalProvider(
                    import.provider(),
                ))?;
            if let Some(target) = terminal.semantic_target(import.subject())? {
                let relation = LayoutAbiDependencyV1::new(import.provider(), target);

                if self.semantic.binary_search(&relation).is_err() {
                    return Err(LayoutAbiSectionError::MissingPhysicalSemantic(relation));
                }
            }
        }
        Ok(PhysicalImportsReplayedLayoutAbiSectionV1 {
            exports: self.exports,
            semantic: self.semantic,
            physical,
        })
    }
}

impl PhysicalImportsReplayedLayoutAbiSectionV1 {
    pub const fn exports(&self) -> &LayoutAbiExportConstituentsV1 {
        &self.exports
    }
    pub fn selected_relations(&self) -> &[LayoutAbiDependencyV1] {
        &self.semantic
    }
    pub const fn physical_imports(&self) -> &CanonicalExternalShapeLinkImportsV1 {
        &self.physical
    }
}
