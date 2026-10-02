//! Current-Cone sources and compiler protocols for shared HIR lowering.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::ConeIdentity;
use std::collections::BTreeMap;

/// Compiler protocols come from current declarations or one imported library.
pub enum CoreProtocolInput {
    CurrentDeclarations,
    Imported(Box<hir::ImportedCoreInputs>),
}

impl From<hir::ImportedCoreInputs> for CoreProtocolInput {
    fn from(inputs: hir::ImportedCoreInputs) -> Self {
        Self::Imported(Box::new(inputs))
    }
}

/// Parsed sources paired with their protocols and matching semantic world.
pub struct CurrentConeSources<'input, 'world> {
    sources: &'input ast::CurrentConeParsedSources,
    core: CoreProtocolInput,
    world: &'world hir::ImportedSemanticWorld<'input>,
    source_names: BTreeMap<ConeIdentity, String>,
}

impl<'input, 'world> CurrentConeSources<'input, 'world> {
    pub fn try_new(
        sources: &'input ast::CurrentConeParsedSources,
        core: impl Into<CoreProtocolInput>,
        world: &'world hir::ImportedSemanticWorld<'input>,
    ) -> Result<Self, CurrentConeSourceError> {
        let core = core.into();
        match (&core, sources.cone() == ConeIdentity::CORE) {
            (CoreProtocolInput::CurrentDeclarations, false) => {
                return Err(CurrentConeSourceError::CurrentProtocolsInOrdinaryCone(
                    sources.cone(),
                ));
            }
            (CoreProtocolInput::Imported(_), true) => {
                return Err(CurrentConeSourceError::CoreImportsOwnProtocols);
            }
            _ => {}
        }
        if world.current() != sources.cone() {
            return Err(CurrentConeSourceError::SemanticWorldCurrentConeMismatch {
                sources: sources.cone(),
                world: world.current(),
            });
        }
        Ok(Self {
            sources,
            core,
            world,
            source_names: BTreeMap::new(),
        })
    }

    /// Cone coordinate labels used to display canonical semantic source paths.
    pub fn with_source_names(mut self, names: BTreeMap<ConeIdentity, String>) -> Self {
        self.source_names = names;
        self
    }

    pub(crate) fn source_names(&self) -> &BTreeMap<ConeIdentity, String> {
        &self.source_names
    }

    pub const fn current_cone(&self) -> ConeIdentity {
        self.sources.cone()
    }

    pub const fn semantic_world(&self) -> &hir::ImportedSemanticWorld<'input> {
        self.world
    }

    pub(crate) const fn sources(&self) -> &'input ast::CurrentConeParsedSources {
        self.sources
    }

    pub(crate) const fn core(&self) -> &CoreProtocolInput {
        &self.core
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentConeSourceError {
    CoreImportsOwnProtocols,
    CurrentProtocolsInOrdinaryCone(ConeIdentity),
    SemanticWorldCurrentConeMismatch {
        sources: ConeIdentity,
        world: ConeIdentity,
    },
}

impl std::fmt::Display for CurrentConeSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CoreImportsOwnProtocols => formatter.write_str(
                "the current core declarations cannot import their own compiler protocols",
            ),
            Self::CurrentProtocolsInOrdinaryCone(cone) => {
                write!(formatter, "Cone {cone} must import its compiler protocols")
            }
            Self::SemanticWorldCurrentConeMismatch { sources, world } => write!(
                formatter,
                "current HIR sources belong to Cone {sources}, but the semantic world belongs to Cone {world}",
            ),
        }
    }
}

impl std::error::Error for CurrentConeSourceError {}
