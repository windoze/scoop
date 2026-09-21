//! Validated production inputs for ordinary-Cone HIR lowering.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::ConeIdentity;

/// Parsed current-Cone sources paired with core type/protocol bindings and
/// the shared dependency semantic world.
///
/// The semantic world is kept beside the source/core input so production
/// lowering cannot accidentally resolve dependency imports against a world
/// belonging to another current Cone. It remains read-only throughout HIR
/// lowering; candidate selection sidecars are introduced separately.
pub struct OrdinarySources<'input, 'world> {
    sources: &'input ast::CurrentConeParsedSources,
    core: hir::ImportedCoreInputs,
    world: &'world hir::ImportedSemanticWorld<'input>,
}

impl<'input, 'world> OrdinarySources<'input, 'world> {
    pub fn try_new(
        sources: &'input ast::CurrentConeParsedSources,
        core: hir::ImportedCoreInputs,
        world: &'world hir::ImportedSemanticWorld<'input>,
    ) -> Result<Self, OrdinarySourceError> {
        if sources.cone() == ConeIdentity::CORE {
            return Err(OrdinarySourceError::CurrentConeIsCore);
        }
        if world.current() != sources.cone() {
            return Err(OrdinarySourceError::SemanticWorldCurrentConeMismatch {
                sources: sources.cone(),
                world: world.current(),
            });
        }
        Ok(Self {
            sources,
            core,
            world,
        })
    }

    pub const fn current_cone(&self) -> ConeIdentity {
        self.sources.cone()
    }

    pub const fn semantic_world(&self) -> &hir::ImportedSemanticWorld<'input> {
        self.world
    }

    pub fn core_compiler_operation_count(&self) -> usize {
        self.core.protocols().compiler_operations().len()
    }

    pub(crate) const fn sources(&self) -> &'input ast::CurrentConeParsedSources {
        self.sources
    }

    pub(crate) const fn core(&self) -> &hir::ImportedCoreInputs {
        &self.core
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
