//! Authority for exported compile-time constant validation.

use std::fmt;

use scoop_hir::{
    CanonicalConstValueKindV1, ConstPropertyDeclarationSourceV1, CoreHirInterfaceBranchV1,
    ExportConstValueSemanticAuthority, PropertyInterfaceRecordV1,
};
use scoop_identity::{
    ConeIdentity, DefinitionOriginSubject, IdentityReferenceError, PersistentPropertyId,
    PropertyOwner, SourceDeclarationKey,
};

use super::CanonicalCrossConeHirSurfaceAuthority;

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

    fn canonical_const_value_type(
        &mut self,
        kind: CanonicalConstValueKindV1,
    ) -> Result<scoop_identity::PersistentTypeId, CrossConeHirConstAuthorityError> {
        let core = self
            .trusted_core()
            .ok_or(CrossConeHirConstAuthorityError::MissingTrustedCore)?;
        let CoreHirInterfaceBranchV1::Core(interface) = core.core_interface() else {
            return Err(CrossConeHirConstAuthorityError::InvalidTrustedCore);
        };
        Ok(interface.compiler_protocols().const_value_source_type(kind))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeHirConstAuthorityError {
    Identity(IdentityReferenceError),
    MissingDefinitionOrigin { property: PersistentPropertyId },
    MissingPropertyInterface { property: PersistentPropertyId },
    MissingTrustedCore,
    InvalidTrustedCore,
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
            Self::MissingTrustedCore => formatter
                .write_str("trusted core is absent from the exported-constant dependency closure"),
            Self::InvalidTrustedCore => {
                formatter.write_str("the canonical core provider has no trusted core interface")
            }
        }
    }
}

impl std::error::Error for CrossConeHirConstAuthorityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::MissingDefinitionOrigin { .. }
            | Self::MissingPropertyInterface { .. }
            | Self::MissingTrustedCore
            | Self::InvalidTrustedCore => None,
        }
    }
}
