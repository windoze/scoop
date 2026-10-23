//! Current-Cone sources and compiler protocols for shared HIR lowering.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::ConeIdentity;
use std::collections::{BTreeMap, BTreeSet};

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
    test_dependencies: BTreeSet<ConeIdentity>,
    documentation: Option<Vec<ast::SourceDocumentation>>,
}

impl<'input, 'world> CurrentConeSources<'input, 'world> {
    pub fn try_new(
        sources: &'input ast::CurrentConeParsedSources,
        core: impl Into<CoreProtocolInput>,
        world: &'world hir::ImportedSemanticWorld<'input>,
    ) -> Result<Self, CurrentConeSourceError> {
        let core = core.into();
        validate_protocol_context(sources.cone(), &core, world)?;
        Ok(Self {
            sources,
            core,
            world,
            source_names: BTreeMap::new(),
            test_dependencies: BTreeSet::new(),
            documentation: None,
        })
    }

    /// Cone coordinate labels used to display canonical semantic source paths.
    pub fn with_source_names(mut self, names: BTreeMap<ConeIdentity, String>) -> Self {
        self.source_names = names;
        self
    }

    pub fn with_test_dependencies(
        mut self,
        dependencies: impl IntoIterator<Item = ConeIdentity>,
    ) -> Result<Self, CurrentConeSourceError> {
        extend_test_dependencies(self.world, &mut self.test_dependencies, dependencies)?;
        Ok(self)
    }

    pub(crate) fn test_dependencies(&self) -> &BTreeSet<ConeIdentity> {
        &self.test_dependencies
    }

    pub(crate) fn source_names(&self) -> &BTreeMap<ConeIdentity, String> {
        &self.source_names
    }

    pub fn with_documentation(
        mut self,
        documentation: Vec<ast::IdentifiedSourceDocumentation>,
    ) -> Result<Self, CurrentConeSourceError> {
        if documentation.len() != self.sources.iter().count()
            || !documentation
                .iter()
                .zip(self.sources.iter())
                .all(|(doc, source)| &doc.source == source.text().identity())
        {
            return Err(CurrentConeSourceError::DocumentationSourcesMismatch);
        }
        self.documentation = Some(
            documentation
                .into_iter()
                .map(|source| source.documentation)
                .collect(),
        );
        Ok(self)
    }

    pub(crate) fn documentation(&self) -> Option<&[ast::SourceDocumentation]> {
        self.documentation.as_deref()
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

pub(crate) fn extend_test_dependencies(
    world: &hir::ImportedSemanticWorld<'_>,
    selected: &mut BTreeSet<ConeIdentity>,
    dependencies: impl IntoIterator<Item = ConeIdentity>,
) -> Result<(), CurrentConeSourceError> {
    for dependency in dependencies {
        if dependency == ConeIdentity::CORE {
            return Err(CurrentConeSourceError::TestOnlyCore);
        }
        if world.direct_provider(dependency).is_none() {
            return Err(CurrentConeSourceError::TestDependencyNotDirect(dependency));
        }
        selected.insert(dependency);
    }
    Ok(())
}

pub(crate) fn validate_protocol_context(
    current: ConeIdentity,
    core: &CoreProtocolInput,
    world: &hir::ImportedSemanticWorld<'_>,
) -> Result<(), CurrentConeSourceError> {
    match (core, current == ConeIdentity::CORE) {
        (CoreProtocolInput::CurrentDeclarations, false) => {
            return Err(CurrentConeSourceError::CurrentProtocolsInOrdinaryCone(
                current,
            ));
        }
        (CoreProtocolInput::Imported(_), true) => {
            return Err(CurrentConeSourceError::CoreImportsOwnProtocols);
        }
        _ => {}
    }
    if world.current() != current {
        return Err(CurrentConeSourceError::SemanticWorldCurrentConeMismatch {
            sources: current,
            world: world.current(),
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentConeSourceError {
    DocumentationSourcesMismatch,
    TestOnlyCore,
    TestDependencyNotDirect(ConeIdentity),
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
            Self::DocumentationSourcesMismatch => formatter
                .write_str("documentation must describe the selected parsed sources in order"),
            Self::TestOnlyCore => {
                formatter.write_str("core is a production dependency, not a test-only dependency")
            }
            Self::TestDependencyNotDirect(cone) => write!(
                formatter,
                "test dependency {cone} is not a direct dependency of the current Cone"
            ),
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
