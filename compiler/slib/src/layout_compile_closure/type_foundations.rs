//! Fact and representation replay scoped to each actual dependency closure.

use scoop_hir::{
    CheckedSharedTypeFoundationV1, CrossConeTypeSemanticsSectionV1, SharedTypeMetadataError,
    SharedTypeMetadataV1,
};
use scoop_identity::ConeIdentity;
use scoop_wire::WirePath;

use super::HirDeclarationsValidatedCrossConeLayoutClosure;
use crate::dependency_reachability::transitive_positions;

impl HirDeclarationsValidatedCrossConeLayoutClosure<'_> {
    /// Borrows the artifact's records to check type facts, representation,
    /// inheritance, slots and defaults before resolving selected uses.
    pub fn validate_type_foundations(
        &mut self,
    ) -> Result<Vec<CheckedSharedTypeFoundationV1<'_>>, CrossConeLayoutTypeFoundationError> {
        let (artifacts, dependencies) = self.declarations.hir_semantic_validation_parts();
        let mut checked: Vec<CheckedSharedTypeFoundationV1<'_>> = Vec::new();
        for (position, artifact) in artifacts.iter_mut().enumerate() {
            let provider = artifact.identity();
            let (identities, foundation, _, public, types) = artifact.hir_semantic_parts();
            let input = SharedTypeMetadataV1 {
                provider,
                identities,
                foundation,
                public,
            };
            let result = validate_provider(types, input, &checked, position, dependencies)
                .map_err(|source| CrossConeLayoutTypeFoundationError {
                    provider,
                    source: Box::new(source),
                })?;
            scoop_wire::allocation::try_reserve(&mut checked, 1, &WirePath::root()).map_err(
                |source| CrossConeLayoutTypeFoundationError {
                    provider,
                    source: Box::new(source.into()),
                },
            )?;
            checked.push(result);
        }
        if let Some((root, dependencies)) = checked.split_last() {
            root.with_inheritance_graph(dependencies, |_| ())
                .map_err(|source| CrossConeLayoutTypeFoundationError {
                    provider: root.provider(),
                    source: Box::new(source),
                })?;
        }
        Ok(checked)
    }
}

fn validate_provider<'a>(
    types: &'a CrossConeTypeSemanticsSectionV1,
    input: SharedTypeMetadataV1<'a>,
    checked: &[CheckedSharedTypeFoundationV1<'a>],
    position: usize,
    dependencies: &[Vec<usize>],
) -> Result<CheckedSharedTypeFoundationV1<'a>, SharedTypeMetadataError> {
    let path = WirePath::root();
    let reachable = transitive_positions(position, dependencies)?;
    let mut providers = Vec::new();
    scoop_wire::allocation::try_reserve(&mut providers, reachable.len(), &path)?;
    providers.extend(reachable.iter().map(|position| checked[*position]));
    let checked = types.validate_shared_foundation(input, &providers)?;
    checked.validate_materialized_type_uses(&providers)?;
    Ok(checked)
}

#[derive(Debug)]
pub struct CrossConeLayoutTypeFoundationError {
    pub provider: ConeIdentity,
    pub source: Box<SharedTypeMetadataError>,
}

impl std::fmt::Display for CrossConeLayoutTypeFoundationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid shared type foundation for {}: {}",
            self.provider, self.source
        )
    }
}

impl std::error::Error for CrossConeLayoutTypeFoundationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
