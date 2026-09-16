use scoop_identity::{
    ConeIdentity, DeclarationScope, DefinitionOrigin, PersistentPropertyId, PersistentTypeId,
    PropertyOwner, SignatureTypeKey, SourceDeclarationKey, SourceDeclarationKind,
};

use super::ExportConstValueV1;
use crate::{CanonicalConstValueKindV1, PropertyInterfaceRecordV1, PropertyRepresentationV1};

mod errors;

pub use errors::ExportConstValueSemanticValidationError;

/// Canonical foundation facts for one ordinary const property.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstPropertyDeclarationSourceV1 {
    declaration: SourceDeclarationKey,
    definition_origin: DefinitionOrigin,
}

impl ConstPropertyDeclarationSourceV1 {
    pub fn new(declaration: SourceDeclarationKey, definition_origin: DefinitionOrigin) -> Self {
        Self {
            declaration,
            definition_origin,
        }
    }

    pub const fn declaration(&self) -> &SourceDeclarationKey {
        &self.declaration
    }

    pub const fn definition_origin(&self) -> &DefinitionOrigin {
        &self.definition_origin
    }
}

/// Supplies current-foundation facts, validated property interfaces, and
/// trusted canonical core owners for exported const values.
pub trait ExportConstValueSemanticAuthority<E> {
    fn current_cone(&self) -> ConeIdentity;

    fn const_property_declaration_source(
        &mut self,
        property: PersistentPropertyId,
    ) -> Result<ConstPropertyDeclarationSourceV1, E>;

    /// Returns the already semantically validated interface selected by the
    /// exact ordinary-property id.
    fn validated_property_interface(
        &mut self,
        property: PersistentPropertyId,
    ) -> Result<&PropertyInterfaceRecordV1, E>;

    /// Returns the canonical non-generic core nominal for one closed value
    /// kind without consulting display names or representation layout.
    fn canonical_const_value_type(
        &mut self,
        kind: CanonicalConstValueKindV1,
    ) -> Result<PersistentTypeId, E>;
}

impl ExportConstValueV1 {
    pub fn validate_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), ExportConstValueSemanticValidationError<E>>
    where
        A: ExportConstValueSemanticAuthority<E>,
    {
        let current_cone = authority.current_cone();
        let source = authority
            .const_property_declaration_source(self.property)
            .map_err(ExportConstValueSemanticValidationError::Declaration)?;
        self.validate_declaration_source(current_cone, &source)?;

        let interface = authority
            .validated_property_interface(self.property)
            .map_err(ExportConstValueSemanticValidationError::PropertyInterface)?;
        self.validate_property_interface(interface)?;

        let kind = self.value.kind();
        let expected_owner = authority
            .canonical_const_value_type(kind)
            .map_err(
                |error| ExportConstValueSemanticValidationError::CanonicalValueType { kind, error },
            )?;
        let expected = SignatureTypeKey::Nominal(expected_owner);
        if self.value_type != expected {
            return Err(
                ExportConstValueSemanticValidationError::ValueKindTypeMismatch {
                    kind,
                    expected: Box::new(expected),
                    actual: Box::new(self.value_type.clone()),
                },
            );
        }
        Ok(())
    }

    fn validate_declaration_source<E>(
        &self,
        current_cone: ConeIdentity,
        source: &ConstPropertyDeclarationSourceV1,
    ) -> Result<(), ExportConstValueSemanticValidationError<E>> {
        let declaration = source.declaration();
        if declaration.declaration_kind() != SourceDeclarationKind::Property {
            return Err(ExportConstValueSemanticValidationError::DeclarationKind {
                actual: declaration.declaration_kind(),
            });
        }
        let actual_property = PersistentPropertyId::from_source_declaration(declaration)
            .map_err(ExportConstValueSemanticValidationError::DeclarationIdentity)?;
        if actual_property != self.property {
            return Err(
                ExportConstValueSemanticValidationError::DeclarationIdentityMismatch {
                    expected: self.property,
                    actual: actual_property,
                },
            );
        }
        if declaration.origin() != current_cone {
            return Err(ExportConstValueSemanticValidationError::DeclarationCone {
                expected: current_cone,
                actual: declaration.origin(),
            });
        }
        if declaration.scope() != &DeclarationScope::ConeWide {
            return Err(ExportConstValueSemanticValidationError::DeclarationScope {
                actual: declaration.scope().clone(),
            });
        }

        let expected_origin = source.definition_origin();
        let actual_cone = expected_origin.source().cone();
        if actual_cone != current_cone {
            return Err(
                ExportConstValueSemanticValidationError::DefinitionOriginCone {
                    expected: current_cone,
                    actual: actual_cone,
                },
            );
        }
        let actual_origin = self.definition_origin.origin();
        if actual_origin != expected_origin {
            return Err(
                ExportConstValueSemanticValidationError::DefinitionOriginMismatch {
                    expected: Box::new(expected_origin.clone()),
                    actual: Box::new(actual_origin.clone()),
                },
            );
        }
        Ok(())
    }

    fn validate_property_interface<E>(
        &self,
        interface: &PropertyInterfaceRecordV1,
    ) -> Result<(), ExportConstValueSemanticValidationError<E>> {
        let expected_declaration = PropertyOwner::Property(self.property);
        if interface.declaration() != expected_declaration {
            return Err(
                ExportConstValueSemanticValidationError::PropertyInterfaceDeclaration {
                    expected: expected_declaration,
                    actual: interface.declaration(),
                },
            );
        }
        if interface.representation() != PropertyRepresentationV1::Const {
            return Err(
                ExportConstValueSemanticValidationError::PropertyRepresentation {
                    actual: interface.representation(),
                },
            );
        }
        // A constructed Const property interface is structurally guaranteed
        // to be read-only, binder-free, receiver-free, and direct-only.
        if self.value_type != *interface.value_type() {
            return Err(
                ExportConstValueSemanticValidationError::PropertyValueTypeMismatch {
                    expected: Box::new(interface.value_type().clone()),
                    actual: Box::new(self.value_type.clone()),
                },
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
