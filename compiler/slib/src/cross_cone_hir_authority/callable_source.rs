//! Authority for source-call protocol validation.

use std::fmt;

use scoop_hir::{
    CallableDeclarationId, CallableSourceInterfaceSemanticAuthority, CoreHirInterfaceBranchV1,
    ExportDefinitionSourceV1,
};
use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DefinitionOriginSubject, PersistentGenericTypeId,
};

use super::{CanonicalCrossConeHirSurfaceAuthority, CrossConeHirDefinitionSourceAuthorityError};

impl CallableSourceInterfaceSemanticAuthority<CrossConeHirCallableSourceAuthorityError>
    for CanonicalCrossConeHirSurfaceAuthority<'_>
{
    fn current_cone(&self) -> ConeIdentity {
        self.current
    }

    fn canonical_array_type(
        &mut self,
    ) -> Result<PersistentGenericTypeId, CrossConeHirCallableSourceAuthorityError> {
        let core = if self.current == ConeIdentity::CORE {
            self.current_core
        } else {
            self.dependencies
                .iter()
                .find(|provider| provider.identity == ConeIdentity::CORE)
                .map(|provider| provider.core)
                .ok_or(CrossConeHirCallableSourceAuthorityError::MissingTrustedCore)?
        };
        let CoreHirInterfaceBranchV1::Core(interface) = core.core_interface() else {
            return Err(CrossConeHirCallableSourceAuthorityError::InvalidTrustedCore);
        };
        Ok(interface.compiler_protocols().array_source_type())
    }

    fn validate_source_parameter_origin(
        &mut self,
        owner: CallableDeclarationId,
        position: u32,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), CrossConeHirCallableSourceAuthorityError> {
        let subject = callable_origin_subject(owner)?;
        let declaration = self
            .current_foundation
            .definition_origin(subject)
            .ok_or(CrossConeHirCallableSourceAuthorityError::MissingDeclarationOrigin { owner })?;
        if declaration.origin().source() != origin.origin().source() {
            return Err(
                CrossConeHirCallableSourceAuthorityError::ParameterSourceMismatch {
                    owner,
                    position,
                },
            );
        }
        self.validate_current_definition_source(origin)
            .map_err(CrossConeHirCallableSourceAuthorityError::DefinitionSource)
    }
}

fn callable_origin_subject(
    owner: CallableTemplateOrigin,
) -> Result<DefinitionOriginSubject, CrossConeHirCallableSourceAuthorityError> {
    match owner {
        CallableTemplateOrigin::Function(id) => Ok(DefinitionOriginSubject::Function(id)),
        CallableTemplateOrigin::GenericFunction(id) => {
            Ok(DefinitionOriginSubject::GenericFunction(id))
        }
        CallableTemplateOrigin::Constructor(id) => Ok(DefinitionOriginSubject::Constructor(id)),
        CallableTemplateOrigin::VariantConstructor(id) => {
            Ok(DefinitionOriginSubject::EnumVariant(id))
        }
        CallableTemplateOrigin::Accessor(_) => {
            Err(CrossConeHirCallableSourceAuthorityError::AccessorOwner)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeHirCallableSourceAuthorityError {
    MissingTrustedCore,
    InvalidTrustedCore,
    AccessorOwner,
    MissingDeclarationOrigin {
        owner: CallableTemplateOrigin,
    },
    ParameterSourceMismatch {
        owner: CallableTemplateOrigin,
        position: u32,
    },
    DefinitionSource(CrossConeHirDefinitionSourceAuthorityError),
}

impl fmt::Display for CrossConeHirCallableSourceAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTrustedCore => formatter.write_str(
                "trusted core is absent from the callable source-interface dependency closure",
            ),
            Self::InvalidTrustedCore => {
                formatter.write_str("the canonical core provider has no trusted core interface")
            }
            Self::AccessorOwner => {
                formatter.write_str("a property accessor cannot own a source-call interface")
            }
            Self::MissingDeclarationOrigin { owner } => write!(
                formatter,
                "callable source-interface owner {owner:?} has no foundation definition origin"
            ),
            Self::ParameterSourceMismatch { owner, position } => write!(
                formatter,
                "source parameter {position} of {owner:?} belongs to a different source"
            ),
            Self::DefinitionSource(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirCallableSourceAuthorityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DefinitionSource(error) => Some(error),
            Self::MissingTrustedCore
            | Self::InvalidTrustedCore
            | Self::AccessorOwner
            | Self::MissingDeclarationOrigin { .. }
            | Self::ParameterSourceMismatch { .. } => None,
        }
    }
}
