//! Saved HIR declarations and the alias expansions needed by downstream stages.

use scoop_identity::ConeIdentity;
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::WirePath;

use super::HirProductionValidatedCrossConeLayoutClosure;
use crate::HirProductionValidatedCrossConeLayoutSections;
use crate::dependency_reachability::transitive_positions;

mod errors;
use CrossConeHirDeclarationValidationError as Error;
pub use errors::{CrossConeHirDeclarationValidationError, CrossConeLayoutHirDeclarationError};

/// Actual artifact metadata whose shared declarations and references passed
/// ordinary HIR checks. Type semantics and machine relations remain pending.
pub struct HirDeclarationsValidatedCrossConeLayoutClosure<'input> {
    pub(super) declarations: HirProductionValidatedCrossConeLayoutClosure<'input>,
    pub(super) aliases: Vec<std::rc::Rc<scoop_hir::CanonicalTypeAliasExpansionsV1>>,
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
    /// Reads alias expansions from already compiled declaration metadata.
    pub fn validate_hir_declarations(
        mut self,
    ) -> Result<
        HirDeclarationsValidatedCrossConeLayoutClosure<'input>,
        CrossConeLayoutHirDeclarationError,
    > {
        let (artifacts, dependencies) = self.hir_semantic_validation_parts();
        let mut aliases: Vec<std::rc::Rc<scoop_hir::CanonicalTypeAliasExpansionsV1>> = Vec::new();
        for (position, artifact) in artifacts.iter_mut().enumerate() {
            let provider = artifact.identity();
            let mut expand = || -> Result<_, Error> {
                let (identities, _, _, interface, _) = artifact.hir_semantic_parts();
                crate::cross_cone_closure::validate_alias_targets(provider, interface, identities)
                    .map_err(Error::AliasReferences)?;
                let reachable =
                    transitive_positions(position, dependencies).map_err(Error::Resource)?;
                let dependencies = reachable
                    .iter()
                    .map(|position| aliases[*position].as_ref())
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
            aliases.push(std::rc::Rc::new(expanded));
        }
        Ok(HirDeclarationsValidatedCrossConeLayoutClosure {
            declarations: self,
            aliases,
        })
    }
}
