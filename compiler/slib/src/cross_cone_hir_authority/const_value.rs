//! Authority for exported compile-time constant validation.

use std::fmt;

use scoop_hir::{
    CanonicalConstValueKindV1, ConstPropertyDeclarationSourceV1, ExportConstValueSemanticAuthority,
    IntrinsicTypeKind, PropertyInterfaceRecordV1, SourceNominalId,
};
use scoop_identity::{
    ConeIdentity, DefinitionOriginSubject, IdentityReferenceError, PersistentPropertyId,
    PropertyOwner, SourceDeclarationKey,
};

use super::{CanonicalCrossConeHirSurfaceAuthority, CrossConeHirIntrinsicTypeError};

impl ExportConstValueSemanticAuthority<CrossConeHirConstAuthorityError>
    for CanonicalCrossConeHirSurfaceAuthority<'_>
{
    fn current_cone(&self) -> ConeIdentity {
        self.current
    }

    fn const_property_declaration_source(
        &mut self,
        property: PersistentPropertyId,
    ) -> Result<ConstPropertyDeclarationSourceV1, CrossConeHirConstAuthorityError> {
        let declaration = self
            .identities
            .canonical_key::<PersistentPropertyId, SourceDeclarationKey>(property)
            .map_err(CrossConeHirConstAuthorityError::Identity)?;
        let definition_origin = self
            .current_foundation
            .definition_origin(DefinitionOriginSubject::Property(property))
            .ok_or(CrossConeHirConstAuthorityError::MissingDefinitionOrigin { property })?;
        Ok(ConstPropertyDeclarationSourceV1::new(
            declaration.as_ref().clone(),
            definition_origin.origin().clone(),
        ))
    }

    fn validated_property_interface(
        &mut self,
        property: PersistentPropertyId,
    ) -> Result<&PropertyInterfaceRecordV1, CrossConeHirConstAuthorityError> {
        self.current_interface
            .property_interfaces()
            .get(PropertyOwner::Property(property))
            .ok_or(CrossConeHirConstAuthorityError::MissingPropertyInterface { property })
    }

    fn validate_const_value_type(
        &mut self,
        value_type: scoop_identity::PersistentTypeId,
        kind: CanonicalConstValueKindV1,
    ) -> Result<(), CrossConeHirConstAuthorityError> {
        let family = match kind {
            CanonicalConstValueKindV1::Integer(kind) => IntrinsicTypeKind::Integer(kind),
            CanonicalConstValueKindV1::Char => IntrinsicTypeKind::Char,
            CanonicalConstValueKindV1::Float(kind) => IntrinsicTypeKind::Float(kind),
            CanonicalConstValueKindV1::Boolean => IntrinsicTypeKind::Boolean,
            CanonicalConstValueKindV1::String => IntrinsicTypeKind::String,
        };
        self.validate_intrinsic_type(SourceNominalId::Concrete(value_type), family)
            .map_err(CrossConeHirConstAuthorityError::ValueType)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeHirConstAuthorityError {
    Identity(IdentityReferenceError),
    MissingDefinitionOrigin { property: PersistentPropertyId },
    MissingPropertyInterface { property: PersistentPropertyId },
    ValueType(CrossConeHirIntrinsicTypeError),
}

impl fmt::Display for CrossConeHirConstAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(formatter),
            Self::MissingDefinitionOrigin { property } => write!(
                formatter,
                "const property {property} has no foundation definition origin"
            ),
            Self::MissingPropertyInterface { property } => write!(
                formatter,
                "const property {property} has no validated public property interface"
            ),
            Self::ValueType(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirConstAuthorityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::ValueType(error) => Some(error),
            Self::MissingDefinitionOrigin { .. } | Self::MissingPropertyInterface { .. } => None,
        }
    }
}
