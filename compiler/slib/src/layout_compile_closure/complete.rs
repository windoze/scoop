//! Complete shared semantic replay before view-specific physical validation.

use super::*;
use crate::{
    CrossConeClosureFoundationError, CrossConeClosureHirProductionError,
    CrossConeClosureIdentityError,
};

impl<'input> DecodedCrossConeLayoutCompileClosure<'input> {
    /// Checks the shared semantic records in the decoded dependency closure.
    /// Link consumption additionally checks objects and physical imports.
    pub fn replay_semantics(
        self,
    ) -> Result<
        LirDependencyGraphReplayedCrossConeLayoutClosure<'input>,
        CrossConeLayoutSemanticClosureError,
    > {
        Ok(self
            .validate_profile_graph()?
            .validate_identities()?
            .validate_foundation_structure()?
            .resolve_hir_sections()?
            .validate_hir_productions()?
            .validate_hir_declarations()?
            .validate_mir_types()?
            .validate_source_callables()?
            .validate_lir_layouts()?
            .validate_ordinary_lir_bridges()?
            .validate_lir_callable_abis()?
            .validate_lir_dispatch()?
            .validate_lir_descriptors()?
            .validate_lir_shape_support()?
            .replay_lir_strong_production()?
            .replay_mir_dependency_graph()?
            .replay_lir_dependency_graph()?)
    }
}

macro_rules! semantic_errors {
    ($($variant:ident($source:ty)),+ $(,)?) => {
        #[derive(Debug)]
        pub enum CrossConeLayoutSemanticClosureError {
            $($variant(Box<$source>)),+
        }

        $(impl From<$source> for CrossConeLayoutSemanticClosureError {
            fn from(source: $source) -> Self {
                Self::$variant(Box::new(source))
            }
        })+

        impl std::error::Error for CrossConeLayoutSemanticClosureError {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match self {
                    $(Self::$variant(source) => Some(source.as_ref())),+
                }
            }
        }
    };
}

semantic_errors! {
    Graph(CrossConeClosureGraphError),
    Identity(CrossConeClosureIdentityError),
    Foundation(CrossConeClosureFoundationError),
    HirResolution(CrossConeLayoutClosureHirResolutionError),
    HirProduction(CrossConeClosureHirProductionError),
    HirDeclarations(CrossConeLayoutHirDeclarationError),
    MirTypes(CrossConeLayoutMirTypesError),
    SourceCallables(CrossConeLayoutMirSourceCallablesError),
    LirLayouts(CrossConeLayoutLirLayoutsError),
    LirCallableAbis(CrossConeLayoutLirCallableAbisError),
    LirDispatch(CrossConeLayoutLirDispatchError),
    LirDescriptors(CrossConeLayoutLirDescriptorsError),
    LirShapeSupport(CrossConeLayoutLirShapeSupportError),
    OrdinaryLirBridges(CrossConeLayoutOrdinaryLirBridgeError),
    LirStrongProduction(CrossConeLayoutLirStrongProductionError),
    MirDependencies(CrossConeLayoutMirDependenciesError),
    LirDependencies(CrossConeLayoutLirDependenciesError),
}

impl std::fmt::Display for CrossConeLayoutSemanticClosureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid shared layout semantics: {self:?}")
    }
}
