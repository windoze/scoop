use scoop_hir::{ImportedProviderInput, ImportedSemanticWorld, ImportedSemanticWorldBuildError};

use super::ValidatedCrossConeSemanticClosure;

impl ValidatedCrossConeSemanticClosure {
    /// Builds shared declaration and source name indexes from this closure.
    pub fn imported_semantic_world(
        &self,
    ) -> Result<ImportedSemanticWorld<'_>, ImportedSemanticWorldBuildError> {
        let mut direct = Vec::with_capacity(self.direct.len());
        let mut support = Vec::with_capacity(
            self.dependency_first
                .len()
                .saturating_sub(self.direct.len()),
        );
        for artifact in &self.dependency_first {
            if artifact.identity() == self.current {
                continue;
            }
            let production = artifact.production();
            let input = ImportedProviderInput {
                foundation: artifact.hir(),
                interface: production.hir_interface(),
                alias_expansions: production.type_alias_expansions(),
            };
            if self.direct.binary_search(&artifact.identity()).is_ok() {
                direct.push(input);
            } else {
                support.push(input);
            }
        }
        ImportedSemanticWorld::from_dependencies(self.current, direct, support)
    }
}
