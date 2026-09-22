//! Authority for source-call protocol validation.

use std::fmt;

use scoop_hir::{
    CallableDeclarationId, CallableSourceInterfaceSemanticAuthority, ExportDefinitionSourceV1,
    IntrinsicTypeKind, SourceNominalId,
};
use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DefinitionOriginSubject, PersistentGenericTypeId,
};

use super::{
    CanonicalCrossConeHirSurfaceAuthority, CrossConeHirDefinitionSourceAuthorityError,
    CrossConeHirIntrinsicTypeError,
};

impl CallableSourceInterfaceSemanticAuthority<CrossConeHirCallableSourceAuthorityError>
    for CanonicalCrossConeHirSurfaceAuthority<'_>
{
    fn current_cone(&self) -> ConeIdentity {
        self.current
    }

    fn validate_array_type(
        &mut self,
        array: PersistentGenericTypeId,
    ) -> Result<(), CrossConeHirCallableSourceAuthorityError> {
        self.validate_intrinsic_type(
            SourceNominalId::GenericTemplate(array),
            IntrinsicTypeKind::Array,
        )
        .map_err(|error| CrossConeHirCallableSourceAuthorityError::ArrayType(Box::new(error)))
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
    ArrayType(Box<CrossConeHirIntrinsicTypeError>),
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
            Self::ArrayType(error) => error.fmt(formatter),
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
            Self::ArrayType(error) => Some(error),
            Self::AccessorOwner
            | Self::MissingDeclarationOrigin { .. }
            | Self::ParameterSourceMismatch { .. } => None,
        }
    }
}
