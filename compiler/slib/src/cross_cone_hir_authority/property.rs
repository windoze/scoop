use scoop_hir::{
    PropertyDeclarationId, PropertyDeclarationIdentityShapeV1, PropertyDeclarationSourceShapeV1,
    PropertyInterfaceSemanticAuthority,
};
use scoop_identity::{
    DuplicateSignatureKey, PersistentPropertyAccessorId, PropertyAccessorKey, PropertyOwner,
    SourceDeclarationKind,
};

use super::{
    CanonicalCrossConeHirSurfaceAuthority, CrossConeHirNominalAuthorityError, optional_signature,
};

impl PropertyInterfaceSemanticAuthority<CrossConeHirNominalAuthorityError>
    for CanonicalCrossConeHirSurfaceAuthority<'_>
{
    fn property_declaration_identity_shape(
        &mut self,
        declaration: PropertyDeclarationId,
    ) -> Result<PropertyDeclarationIdentityShapeV1, CrossConeHirNominalAuthorityError> {
        let key = self.property_key(declaration)?;
        self.require_current("property interface", key.origin())?;
        let expected_kind = match declaration {
            PropertyOwner::Property(_) => SourceDeclarationKind::Property,
            PropertyOwner::ExtensionProperty(_) => SourceDeclarationKind::ExtensionProperty,
        };
        if key.declaration_kind() != expected_kind {
            return Err(
                CrossConeHirNominalAuthorityError::PropertyDeclarationKindMismatch {
                    declaration,
                    actual: key.declaration_kind(),
                },
            );
        }
        let (own_arity, receiver) = match key.duplicate_signature() {
            DuplicateSignatureKey::Property {
                type_parameter_count,
                receiver,
            } => (*type_parameter_count, optional_signature(receiver)),
            actual => {
                return Err(
                    CrossConeHirNominalAuthorityError::PropertySignatureKindMismatch {
                        declaration,
                        actual: actual.clone(),
                    },
                );
            }
        };
        let owner = self.source_key_owner("property interface", &key)?;
        self.require_current_nominal_owner("property owner", owner)?;
        let outer_arity = self.declaration_owner_arity(owner)?;
        Ok(PropertyDeclarationIdentityShapeV1::new(
            owner,
            own_arity,
            outer_arity,
            receiver,
        ))
    }

    fn property_declaration_source_shape(
        &mut self,
        declaration: PropertyDeclarationId,
    ) -> Result<PropertyDeclarationSourceShapeV1, CrossConeHirNominalAuthorityError> {
        // `cross-cone-interface/2` is the persisted definition-side projection;
        // its writer must derive these facts from typed Export HIR. At read time
        // the exact typed record is therefore the canonical source projection,
        // while the later internal-closure pass cross-checks it against callable
        // accessors and const records. Never substitute a name or table position.
        let record = self
            .current_interface
            .property_interfaces()
            .get(declaration)
            .ok_or(CrossConeHirNominalAuthorityError::MissingPropertyInterface { declaration })?;
        Ok(PropertyDeclarationSourceShapeV1::new(
            record.capability(),
            record.representation(),
            record.access(),
        ))
    }

    fn property_accessor_key(
        &mut self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<PropertyAccessorKey, CrossConeHirNominalAuthorityError> {
        self.identities
            .canonical_key::<PersistentPropertyAccessorId, PropertyAccessorKey>(accessor)
            .map(|key| *key.as_ref())
            .map_err(CrossConeHirNominalAuthorityError::Identity)
    }
}
