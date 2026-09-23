//! Shared HIR declaration and reference replay from layout artifact bytes.

use scoop_identity::ConeIdentity;
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::{BudgetMeter, WirePath};

use super::HirProductionValidatedCrossConeLayoutClosure;
use crate::{
    DefinitionSourceProviderView, HirProductionValidatedCrossConeLayoutSections,
    cross_cone_hir_authority::ValidatedNominalProviderView,
    hir_interface_validation::HirInterfaceValidationInput,
    layout_hir_semantics::transitive_positions,
};

mod errors;
use CrossConeHirDeclarationValidationError as Error;
pub use errors::{CrossConeHirDeclarationValidationError, CrossConeLayoutHirDeclarationError};

/// Actual artifact metadata whose shared declarations and references passed
/// ordinary HIR checks. Type semantics and machine relations remain pending.
pub struct HirDeclarationsValidatedCrossConeLayoutClosure<'input>(
    pub(super) HirProductionValidatedCrossConeLayoutClosure<'input>,
);

impl HirDeclarationsValidatedCrossConeLayoutClosure<'_> {
    pub const fn current(&self) -> ConeIdentity {
        self.0.current()
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.0.target_selection()
    }

    pub fn direct_providers(&self) -> &[ConeIdentity] {
        self.0.direct_providers()
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &HirProductionValidatedCrossConeLayoutSections<'_>> {
        self.0.dependency_first()
    }

    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<&HirProductionValidatedCrossConeLayoutSections<'_>> {
        self.0.artifact(provider)
    }
}

impl<'input> HirProductionValidatedCrossConeLayoutClosure<'input> {
    /// Uses only this closure's decoded metadata, actual typed identities and
    /// per-artifact budget. No producer source factory participates in replay.
    pub fn validate_hir_declarations(
        mut self,
    ) -> Result<
        HirDeclarationsValidatedCrossConeLayoutClosure<'input>,
        CrossConeLayoutHirDeclarationError,
    > {
        let (artifacts, dependencies) = self.hir_semantic_validation_parts();
        for position in 0..artifacts.len() {
            let (previous, remaining) = artifacts.split_at_mut(position);
            let artifact = &mut remaining[0];
            let provider = artifact.identity();
            validate_provider(artifact, previous, position, dependencies).map_err(|source| {
                CrossConeLayoutHirDeclarationError {
                    provider,
                    source: Box::new(source),
                }
            })?;
        }
        Ok(HirDeclarationsValidatedCrossConeLayoutClosure(self))
    }
}

fn validate_provider(
    artifact: &mut HirProductionValidatedCrossConeLayoutSections<'_>,
    previous: &[HirProductionValidatedCrossConeLayoutSections<'_>],
    position: usize,
    dependency_positions: &[Vec<usize>],
) -> Result<(), Error> {
    let current = artifact.identity();
    let (identities, foundation, core, interface, _, meter) = artifact.hir_semantic_parts();
    let path = WirePath::root();
    let edge_count = dependency_positions[..=position]
        .iter()
        .fold(0_u64, |count, edges| {
            count.saturating_add(edges.len() as u64)
        });
    meter
        .charge_work(
            (position as u64)
                .saturating_add(edge_count)
                .saturating_add(1),
            &path,
        )
        .map_err(Error::Resource)?;
    let reachable = transitive_positions(position, dependency_positions);
    let dependencies = collect_views(
        reachable
            .iter()
            .map(|position| previous[*position].nominal_provider_view()),
        meter,
    )?;
    let direct = collect_views(
        dependency_positions[position]
            .iter()
            .map(|position| previous[*position].identity()),
        meter,
    )?;
    let definition_sources = collect_views(
        dependencies
            .iter()
            .map(|provider| DefinitionSourceProviderView {
                identity: provider.identity,
                foundation: provider.foundation,
            }),
        meter,
    )?;
    interface
        .validate_internal_closures(core.direct_public_surface(), meter, &path)
        .map_err(|error| Error::Internal(Box::new(error)))?;
    let input = HirInterfaceValidationInput {
        current,
        identities,
        foundation,
        core,
        interface,
    };
    input
        .definition_sources(&definition_sources, meter)
        .map_err(Error::DefinitionSources)?;
    input
        .nominals(copy_dependencies(&dependencies, meter)?, meter)
        .map_err(Error::Nominals)?;
    input
        .properties(copy_dependencies(&dependencies, meter)?, meter)
        .map_err(Error::Properties)?;
    input
        .callables(copy_dependencies(&dependencies, meter)?, meter)
        .map_err(Error::Callables)?;
    input
        .type_aliases(copy_dependencies(&dependencies, meter)?, meter)
        .map_err(Error::TypeAliases)?;
    input
        .sources(copy_dependencies(&dependencies, meter)?, meter)
        .map_err(Error::Sources)?;
    input
        .constants(copy_dependencies(&dependencies, meter)?, meter)
        .map_err(Error::Constants)?;
    input
        .references(&direct, &dependencies, meter)
        .map_err(Error::References)
}

fn copy_dependencies<'a>(
    dependencies: &[ValidatedNominalProviderView<'a>],
    meter: &mut BudgetMeter,
) -> Result<Vec<ValidatedNominalProviderView<'a>>, Error> {
    collect_views(dependencies.iter().copied(), meter)
}

fn collect_views<T>(
    values: impl ExactSizeIterator<Item = T>,
    meter: &mut BudgetMeter,
) -> Result<Vec<T>, Error> {
    let requested_slots = values.len();
    meter
        .charge_work(requested_slots as u64, &WirePath::root())
        .map_err(Error::Resource)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(requested_slots)
        .map_err(|_| Error::Allocation { requested_slots })?;
    result.extend(values);
    Ok(result)
}
