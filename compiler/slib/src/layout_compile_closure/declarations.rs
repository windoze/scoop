//! Shared HIR declaration and reference replay from layout artifact bytes.

use scoop_identity::ConeIdentity;
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::WirePath;

use super::HirProductionValidatedCrossConeLayoutClosure;
use crate::dependency_reachability::transitive_positions;
use crate::{
    DefinitionSourceProviderView, HirProductionValidatedCrossConeLayoutSections,
    cross_cone_hir_authority::ValidatedNominalProviderView,
    hir_interface_validation::HirInterfaceValidationInput,
};

mod errors;
use CrossConeHirDeclarationValidationError as Error;
pub use errors::{CrossConeHirDeclarationValidationError, CrossConeLayoutHirDeclarationError};

/// Actual artifact metadata whose shared declarations and references passed
/// ordinary HIR checks. Type semantics and machine relations remain pending.
pub struct HirDeclarationsValidatedCrossConeLayoutClosure<'input> {
    pub(super) declarations: HirProductionValidatedCrossConeLayoutClosure<'input>,
    pub(super) aliases: Vec<scoop_hir::CanonicalTypeAliasExpansionsV1>,
}

impl HirDeclarationsValidatedCrossConeLayoutClosure<'_> {
    pub const fn current(&self) -> ConeIdentity {
        self.declarations.current()
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.declarations.target_selection()
    }

    pub fn direct_providers(&self) -> &[ConeIdentity] {
        self.declarations.direct_providers()
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &HirProductionValidatedCrossConeLayoutSections<'_>> {
        self.declarations.dependency_first()
    }

    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<&HirProductionValidatedCrossConeLayoutSections<'_>> {
        self.declarations.artifact(provider)
    }
}

impl<'input> HirProductionValidatedCrossConeLayoutClosure<'input> {
    /// Checks declarations using the closure's decoded metadata and typed identities.
    pub fn validate_hir_declarations(
        mut self,
    ) -> Result<
        HirDeclarationsValidatedCrossConeLayoutClosure<'input>,
        CrossConeLayoutHirDeclarationError,
    > {
        let (artifacts, dependencies) = self.hir_semantic_validation_parts();
        let mut aliases = Vec::new();
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
            let mut expand = || -> Result<_, Error> {
                let (identities, _, _, interface, _) = artifact.hir_semantic_parts();
                crate::cross_cone_closure::validate_alias_targets(provider, interface, identities)
                    .map_err(Error::AliasReferences)?;
                let reachable =
                    transitive_positions(position, dependencies).map_err(Error::Resource)?;
                let dependencies = reachable
                    .iter()
                    .map(|position| &aliases[*position])
                    .collect::<Vec<_>>();
                interface
                    .type_aliases()
                    .expand_alias_closure(&dependencies, &WirePath::root().field(5))
                    .map_err(Error::AliasExpansion)
            };
            let expanded = expand().map_err(|source| CrossConeLayoutHirDeclarationError {
                provider,
                source: Box::new(source),
            })?;
            aliases.push(expanded);
        }
        Ok(HirDeclarationsValidatedCrossConeLayoutClosure {
            declarations: self,
            aliases,
        })
    }
}

fn validate_provider(
    artifact: &mut HirProductionValidatedCrossConeLayoutSections<'_>,
    previous: &[HirProductionValidatedCrossConeLayoutSections<'_>],
    position: usize,
    dependency_positions: &[Vec<usize>],
) -> Result<(), Error> {
    let current = artifact.identity();
    let (identities, foundation, core, interface, _) = artifact.hir_semantic_parts();
    let path = WirePath::root();
    let reachable =
        transitive_positions(position, dependency_positions).map_err(Error::Resource)?;
    let dependencies = collect_views(
        reachable
            .iter()
            .map(|position| previous[*position].nominal_provider_view()),
    )?;
    let definition_sources =
        collect_views(
            dependencies
                .iter()
                .map(|provider| DefinitionSourceProviderView {
                    identity: provider.identity,
                    foundation: provider.foundation,
                }),
        )?;
    interface
        .validate_internal_closures(core.direct_public_surface(), &path)
        .map_err(|error| Error::Internal(Box::new(error)))?;
    let input = HirInterfaceValidationInput {
        current,
        identities,
        foundation,
        core,
        interface,
    };
    input
        .definition_sources(&definition_sources)
        .map_err(Error::DefinitionSources)?;
    input
        .nominals(copy_dependencies(&dependencies)?)
        .map_err(Error::Nominals)?;
    input
        .properties(copy_dependencies(&dependencies)?)
        .map_err(Error::Properties)?;
    input
        .callables(copy_dependencies(&dependencies)?)
        .map_err(Error::Callables)?;
    input
        .type_aliases(copy_dependencies(&dependencies)?)
        .map_err(Error::TypeAliases)?;
    input
        .sources(copy_dependencies(&dependencies)?)
        .map_err(Error::Sources)?;
    input
        .constants(copy_dependencies(&dependencies)?)
        .map_err(Error::Constants)?;
    input.references(&dependencies).map_err(Error::References)
}

fn copy_dependencies<'a>(
    dependencies: &[ValidatedNominalProviderView<'a>],
) -> Result<Vec<ValidatedNominalProviderView<'a>>, Error> {
    collect_views(dependencies.iter().copied())
}

fn collect_views<T>(values: impl ExactSizeIterator<Item = T>) -> Result<Vec<T>, Error> {
    let requested_slots = values.len();

    let mut result = Vec::new();
    scoop_wire::allocation::try_reserve(&mut result, requested_slots, &WirePath::root())
        .map_err(Error::Resource)?;
    result.extend(values);
    Ok(result)
}
