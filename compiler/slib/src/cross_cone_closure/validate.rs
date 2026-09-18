//! One-shot orchestration of the cross-Cone semantic closure state machine.

use std::fmt;

use scoop_identity::SemanticIdentitySession;

use super::*;

/// Validates every closure phase and commits all identities atomically.
///
/// Keeping this orchestration in `scoop-slib` prevents the driver and parent
/// scheduler from accidentally skipping a type-state transition when they
/// reopen the same artifact graph for different purposes.
pub fn validate_and_commit_cross_cone_semantic_closure<'input>(
    closure: DecodedCrossConeClosure<'input>,
    session: &mut SemanticIdentitySession,
) -> Result<ValidatedCrossConeSemanticClosure<'input>, CrossConeSemanticClosureValidationError> {
    closure
        .validate_profile_graph()
        .map_err(|source| CrossConeSemanticClosureValidationError::Graph(Box::new(source)))?
        .validate_identities()
        .map_err(|source| CrossConeSemanticClosureValidationError::Identity(Box::new(source)))?
        .validate_foundation_structure()
        .map_err(|source| CrossConeSemanticClosureValidationError::Foundation(Box::new(source)))?
        .resolve_hir_interfaces()
        .map_err(|source| CrossConeSemanticClosureValidationError::HirResolution(Box::new(source)))?
        .validate_hir_productions()
        .map_err(|source| CrossConeSemanticClosureValidationError::HirProduction(Box::new(source)))?
        .validate_internal_hir_closures()
        .map_err(|source| CrossConeSemanticClosureValidationError::InternalHir(Box::new(source)))?
        .validate_definition_sources()
        .map_err(|source| {
            CrossConeSemanticClosureValidationError::DefinitionSource(Box::new(source))
        })?
        .validate_nominal_surfaces()
        .map_err(|source| CrossConeSemanticClosureValidationError::Nominal(Box::new(source)))?
        .validate_property_surfaces()
        .map_err(|source| CrossConeSemanticClosureValidationError::Property(Box::new(source)))?
        .validate_callable_surfaces()
        .map_err(|source| CrossConeSemanticClosureValidationError::Callable(Box::new(source)))?
        .validate_type_alias_surfaces()
        .map_err(|source| CrossConeSemanticClosureValidationError::TypeAlias(Box::new(source)))?
        .validate_source_interfaces()
        .map_err(|source| {
            CrossConeSemanticClosureValidationError::SourceInterface(Box::new(source))
        })?
        .validate_const_values()
        .map_err(|source| CrossConeSemanticClosureValidationError::ConstValue(Box::new(source)))?
        .validate_public_binding_routes()
        .map_err(|source| CrossConeSemanticClosureValidationError::PublicRoute(Box::new(source)))?
        .validate_external_hir_references()
        .map_err(|source| {
            CrossConeSemanticClosureValidationError::ExternalReference(Box::new(source))
        })?
        .validate_and_expand_type_aliases()
        .map_err(|source| {
            CrossConeSemanticClosureValidationError::TypeAliasExpansion(Box::new(source))
        })?
        .validate_mir_bridges()
        .map_err(|source| CrossConeSemanticClosureValidationError::MirBridge(Box::new(source)))?
        .validate_lir_bridges()
        .map_err(|source| CrossConeSemanticClosureValidationError::LirBridge(Box::new(source)))?
        .commit(session)
        .map_err(|source| CrossConeSemanticClosureValidationError::Commit(Box::new(source)))
}

#[derive(Debug)]
pub enum CrossConeSemanticClosureValidationError {
    Graph(Box<CrossConeClosureGraphError>),
    Identity(Box<CrossConeClosureIdentityError>),
    Foundation(Box<CrossConeClosureFoundationError>),
    HirResolution(Box<CrossConeClosureHirResolutionError>),
    HirProduction(Box<CrossConeClosureHirProductionError>),
    InternalHir(Box<CrossConeClosureInternalHirError>),
    DefinitionSource(Box<CrossConeClosureDefinitionSourceError>),
    Nominal(Box<CrossConeClosureNominalSurfaceError>),
    Property(Box<CrossConeClosurePropertySurfaceError>),
    Callable(Box<CrossConeClosureCallableSurfaceError>),
    TypeAlias(Box<CrossConeClosureTypeAliasSurfaceError>),
    SourceInterface(Box<CrossConeClosureSourceInterfaceError>),
    ConstValue(Box<CrossConeClosureConstError>),
    PublicRoute(Box<CrossConeClosurePublicRouteError>),
    ExternalReference(Box<CrossConeClosureExternalReferenceError>),
    TypeAliasExpansion(Box<CrossConeClosureTypeAliasExpansionError>),
    MirBridge(Box<CrossConeClosureMirBridgeError>),
    LirBridge(Box<CrossConeClosureLirBridgeError>),
    Commit(Box<CrossConeSemanticCommitError>),
}

impl fmt::Display for CrossConeSemanticClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid cross-Cone semantic closure: {self:?}")
    }
}

impl std::error::Error for CrossConeSemanticClosureValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Graph(source) => source.as_ref(),
            Self::Identity(source) => source.as_ref(),
            Self::Foundation(source) => source.as_ref(),
            Self::HirResolution(source) => source.as_ref(),
            Self::HirProduction(source) => source.as_ref(),
            Self::InternalHir(source) => source.as_ref(),
            Self::DefinitionSource(source) => source.as_ref(),
            Self::Nominal(source) => source.as_ref(),
            Self::Property(source) => source.as_ref(),
            Self::Callable(source) => source.as_ref(),
            Self::TypeAlias(source) => source.as_ref(),
            Self::SourceInterface(source) => source.as_ref(),
            Self::ConstValue(source) => source.as_ref(),
            Self::PublicRoute(source) => source.as_ref(),
            Self::ExternalReference(source) => source.as_ref(),
            Self::TypeAliasExpansion(source) => source.as_ref(),
            Self::MirBridge(source) => source.as_ref(),
            Self::LirBridge(source) => source.as_ref(),
            Self::Commit(source) => source.as_ref(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_shot_validator_runs_the_complete_empty_core_state_machine() {
        let mut session = SemanticIdentitySession::new();
        let closure = validate_and_commit_cross_cone_semantic_closure(
            DecodedCrossConeClosure::new(
                scoop_identity::ConeIdentity::CORE,
                scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
                Vec::new(),
                Vec::new(),
            ),
            &mut session,
        )
        .unwrap();

        assert_eq!(closure.current(), scoop_identity::ConeIdentity::CORE);
        assert_eq!(closure.provider_count(), 0);
        assert!(closure.current_artifact().is_none());
    }
}
