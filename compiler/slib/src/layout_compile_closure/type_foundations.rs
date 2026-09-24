//! Fact and representation replay scoped to each actual dependency closure.

use scoop_hir::{
    CheckedSharedTypeFoundationV1, CrossConeTypeSemanticsSectionV1, SharedTypeMetadataError,
    SharedTypeMetadataV1,
};
use scoop_identity::ConeIdentity;
use scoop_wire::{BudgetMeter, WirePath};

use super::HirDeclarationsValidatedCrossConeLayoutClosure;
use crate::dependency_reachability::transitive_positions;

impl HirDeclarationsValidatedCrossConeLayoutClosure<'_> {
    /// Borrows checked records from the artifacts themselves. Fact,
    /// representation, inheritance, slot and default replay share the original
    /// artifact budget. The complete selected-use join remains pending.
    pub fn validate_type_foundations(
        &mut self,
    ) -> Result<Vec<CheckedSharedTypeFoundationV1<'_>>, CrossConeLayoutTypeFoundationError> {
        let (artifacts, dependencies) = self.0.hir_semantic_validation_parts();
        let mut checked: Vec<CheckedSharedTypeFoundationV1<'_>> = Vec::new();
        for (position, artifact) in artifacts.iter_mut().enumerate() {
            let provider = artifact.identity();
            let (identities, foundation, _, public, types, meter) = artifact.hir_semantic_parts();
            let input = SharedTypeMetadataV1 {
                provider,
                identities,
                foundation,
                public,
            };
            let result = validate_provider(types, input, &checked, position, dependencies, meter)
                .map_err(|source| CrossConeLayoutTypeFoundationError {
                provider,
                source: Box::new(source),
            })?;
            meter
                .try_reserve_collection_slots(&mut checked, 1, &WirePath::root())
                .map_err(|source| CrossConeLayoutTypeFoundationError {
                    provider,
                    source: Box::new(source.into()),
                })?;
            checked.push(result);
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
    meter: &mut BudgetMeter,
) -> Result<CheckedSharedTypeFoundationV1<'a>, SharedTypeMetadataError> {
    let path = WirePath::root();
    let reachable = transitive_positions(position, dependencies, meter)?;
    let mut providers = Vec::new();
    meter.try_reserve_collection_slots(&mut providers, reachable.len(), &path)?;
    providers.extend(reachable.iter().map(|position| checked[*position]));
    let checked = types.validate_shared_foundation(input, &providers, meter)?;
    checked.with_inheritance_graph(&providers, meter, |_, _| ())?;
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
