//! Validated production inputs for ordinary-Cone HIR lowering.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::ConeIdentity;

/// Parsed current-Cone sources paired with the only imported HIR capability
/// authorized by the M23-3 core-only path.
///
/// The fields remain private so the lowerer always receives the source graph
/// and the atomic trusted core prelude/protocol authority as one
/// lifetime-bound input.
pub struct OrdinaryCoreOnlySources<'a> {
    sources: &'a ast::CurrentConeParsedSources,
    core: hir::ImportedCoreInputs<'a>,
}

impl<'a> OrdinaryCoreOnlySources<'a> {
    pub fn try_new(
        sources: &'a ast::CurrentConeParsedSources,
        core: hir::ImportedCoreInputs<'a>,
    ) -> Result<Self, OrdinaryCoreOnlySourceError> {
        if sources.cone() == ConeIdentity::CORE {
            return Err(OrdinaryCoreOnlySourceError::CurrentConeIsCore);
        }
        Ok(Self { sources, core })
    }

    pub const fn current_cone(&self) -> ConeIdentity {
        self.sources.cone()
    }

    pub fn core_binding_count(&self) -> usize {
        self.core.prelude().bindings().len()
    }

    pub fn core_compiler_operation_count(&self) -> usize {
        self.core.protocols().compiler_operations().len()
    }

    pub(crate) const fn sources(&self) -> &'a ast::CurrentConeParsedSources {
        self.sources
    }

    pub(crate) const fn core(&self) -> &hir::ImportedCoreInputs<'a> {
        &self.core
    }

    pub(crate) fn bind_core_selection(
        &self,
        selection: hir::ImportedCoreSelectionPlan,
    ) -> Result<hir::SelectedImportedCoreSet<'a>, hir::CorePreludeSelectionBindError> {
        self.core.prelude().bind_selection(selection)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrdinaryCoreOnlySourceError {
    CurrentConeIsCore,
}

impl std::fmt::Display for OrdinaryCoreOnlySourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .write_str("ordinary HIR input cannot contain the reserved core Cone as current source")
    }
}

impl std::error::Error for OrdinaryCoreOnlySourceError {}

/// Parsed current-Cone sources paired with the trusted core authority and a
/// validated ordinary dependency semantic world.
///
/// The semantic world is kept beside the source/core input so production
/// lowering cannot accidentally resolve dependency imports against a world
/// belonging to another current Cone. It remains read-only throughout HIR
/// lowering; candidate selection sidecars are introduced separately.
pub struct OrdinarySources<'input, 'world> {
    core_only: OrdinaryCoreOnlySources<'input>,
    world: &'world hir::ImportedSemanticWorld<'input>,
}

impl<'input, 'world> OrdinarySources<'input, 'world> {
    pub fn try_new(
        sources: &'input ast::CurrentConeParsedSources,
        core: hir::ImportedCoreInputs<'input>,
        world: &'world hir::ImportedSemanticWorld<'input>,
    ) -> Result<Self, OrdinarySourceError> {
        let core_only = OrdinaryCoreOnlySources::try_new(sources, core)
            .map_err(|_| OrdinarySourceError::CurrentConeIsCore)?;
        if world.current() != sources.cone() {
            return Err(OrdinarySourceError::SemanticWorldCurrentConeMismatch {
                sources: sources.cone(),
                world: world.current(),
            });
        }
        Ok(Self { core_only, world })
    }

    pub const fn current_cone(&self) -> ConeIdentity {
        self.core_only.current_cone()
    }

    pub const fn semantic_world(&self) -> &hir::ImportedSemanticWorld<'input> {
        self.world
    }

    pub(crate) const fn core_only(&self) -> &OrdinaryCoreOnlySources<'input> {
        &self.core_only
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrdinarySourceError {
    CurrentConeIsCore,
    SemanticWorldCurrentConeMismatch {
        sources: ConeIdentity,
        world: ConeIdentity,
    },
}

impl std::fmt::Display for OrdinarySourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CurrentConeIsCore => formatter.write_str(
                "ordinary HIR input cannot contain the reserved core Cone as current source",
            ),
            Self::SemanticWorldCurrentConeMismatch { sources, world } => write!(
                formatter,
                "ordinary HIR sources belong to Cone {sources}, but the semantic world belongs to Cone {world}",
            ),
        }
    }
}

impl std::error::Error for OrdinarySourceError {}
