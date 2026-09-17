use scoop_hir::{
    CallableDeclarationId, CallableDeclarationIdentityShapeV1, CallableInterfaceSemanticAuthority,
    NominalSourceShapeV1, PublicDeclarationOwnerV1,
};
use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, DuplicateSignatureKey, PersistentEnumVariantId,
    PropertyOwner, SourceDeclarationKind,
};

use super::{
    CanonicalCrossConeHirSurfaceAuthority, CrossConeHirNominalAuthorityError, optional_signature,
};

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    fn function_identity_shape(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> Result<CallableDeclarationIdentityShapeV1, CrossConeHirNominalAuthorityError> {
        let key = self.function_key(declaration)?;
        self.require_current("callable interface", key.origin())?;
        let owner = self.source_key_owner("callable interface", &key)?;
        self.require_current_nominal_owner("callable owner", owner)?;
        let expected_kind = match declaration {
            CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_) => {
                SourceDeclarationKind::Function
            }
            CallableTemplateOrigin::Constructor(_) => SourceDeclarationKind::Constructor,
            CallableTemplateOrigin::Accessor(_) | CallableTemplateOrigin::VariantConstructor(_) => {
                return Err(
                    CrossConeHirNominalAuthorityError::MissingSourceDeclarationKey { declaration },
                );
            }
        };
        if key.declaration_kind() != expected_kind {
            return Err(
                CrossConeHirNominalAuthorityError::CallableDeclarationKindMismatch {
                    declaration,
                    actual: key.declaration_kind(),
                },
            );
        }

        let (own_arity, receiver, parameters) = match (declaration, key.duplicate_signature()) {
            (
                CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_),
                DuplicateSignatureKey::Function {
                    type_parameter_count,
                    receiver,
                    parameters,
                },
            ) => (
                *type_parameter_count,
                optional_signature(receiver),
                parameters.clone(),
            ),
            (
                CallableTemplateOrigin::Constructor(_),
                DuplicateSignatureKey::Constructor { parameters },
            ) => (0, None, parameters.clone()),
            (_, actual) => {
                return Err(
                    CrossConeHirNominalAuthorityError::CallableSignatureKindMismatch {
                        declaration,
                        actual: actual.clone(),
                    },
                );
            }
        };
        if matches!(declaration, CallableTemplateOrigin::Constructor(_))
            && !matches!(owner, PublicDeclarationOwnerV1::Nominal(_))
        {
            return Err(
                CrossConeHirNominalAuthorityError::CallableNominalOwnerRequired {
                    declaration,
                    actual: owner,
                },
            );
        }
        let outer_arity = self.declaration_owner_arity(owner)?;
        Ok(CallableDeclarationIdentityShapeV1::new(
            owner,
            own_arity,
            outer_arity,
            receiver,
            parameters,
        ))
    }

    fn accessor_identity_shape(
        &self,
        accessor: scoop_identity::PersistentPropertyAccessorId,
    ) -> Result<CallableDeclarationIdentityShapeV1, CrossConeHirNominalAuthorityError> {
        let accessor_key = self
            .identities
            .canonical_key::<_, scoop_identity::PropertyAccessorKey>(accessor)
            .map_err(CrossConeHirNominalAuthorityError::Identity)?;
        let declaration = accessor_key.owner();
        let property_key = self.property_key(declaration)?;
        self.require_current("property accessor", property_key.origin())?;
        let expected_kind = match declaration {
            PropertyOwner::Property(_) => SourceDeclarationKind::Property,
            PropertyOwner::ExtensionProperty(_) => SourceDeclarationKind::ExtensionProperty,
        };
        if property_key.declaration_kind() != expected_kind {
            return Err(
                CrossConeHirNominalAuthorityError::PropertyDeclarationKindMismatch {
                    declaration,
                    actual: property_key.declaration_kind(),
                },
            );
        }
        let (property_arity, receiver) = match property_key.duplicate_signature() {
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
        let owner = self.source_key_owner("property accessor", &property_key)?;
        self.require_current_nominal_owner("property owner", owner)?;
        let property = self
            .current_interface
            .property_interfaces()
            .get(declaration)
            .ok_or(CrossConeHirNominalAuthorityError::MissingPropertyInterface { declaration })?;
        let parameters = match accessor_key.role() {
            AccessorRole::Getter => Vec::new(),
            AccessorRole::Setter => vec![property.value_type().clone()],
        };
        let outer_arity = match owner {
            PublicDeclarationOwnerV1::Extension => property_arity,
            PublicDeclarationOwnerV1::Nominal(_) => self.declaration_owner_arity(owner)?,
            PublicDeclarationOwnerV1::TopLevel => 0,
        };
        Ok(CallableDeclarationIdentityShapeV1::new(
            owner,
            0,
            outer_arity,
            receiver,
            parameters,
        ))
    }

    fn variant_constructor_identity_shape(
        &self,
        variant: PersistentEnumVariantId,
    ) -> Result<CallableDeclarationIdentityShapeV1, CrossConeHirNominalAuthorityError> {
        let key = self
            .identities
            .canonical_key::<PersistentEnumVariantId, scoop_identity::EnumVariantIdentityKey>(
                variant,
            )
            .map_err(CrossConeHirNominalAuthorityError::Identity)?;
        let declaration = key
            .source_owner()
            .ok_or(CrossConeHirNominalAuthorityError::GeneratedEnumVariant { variant })?;
        let declaration_key = self.source_nominal_key(declaration)?;
        self.require_current("enum variant constructor", declaration_key.origin())?;
        let shape = self.nominal_shape(declaration)?;
        let record = self
            .current_interface
            .nominal_interfaces()
            .get(declaration)
            .ok_or(CrossConeHirNominalAuthorityError::MissingNominalInterface {
                origin: self.current,
                declaration,
            })?;
        let NominalSourceShapeV1::Enum(source) = record.source_shape() else {
            return Err(
                CrossConeHirNominalAuthorityError::NominalSourceShapeNotEnum {
                    declaration,
                    actual: record.source_shape().kind(),
                },
            );
        };
        let variant_record = source
            .variants()
            .iter()
            .find(|record| record.variant() == variant)
            .ok_or(CrossConeHirNominalAuthorityError::MissingEnumVariant {
                declaration,
                variant,
            })?;
        let parameters = variant_record
            .fields()
            .iter()
            .map(|field| field.value_type().clone())
            .collect();
        Ok(CallableDeclarationIdentityShapeV1::new(
            PublicDeclarationOwnerV1::Nominal(declaration),
            0,
            shape.type_parameter_arity(),
            None,
            parameters,
        ))
    }
}

impl CallableInterfaceSemanticAuthority<CrossConeHirNominalAuthorityError>
    for CanonicalCrossConeHirSurfaceAuthority<'_>
{
    fn callable_declaration_identity_shape(
        &mut self,
        declaration: CallableDeclarationId,
    ) -> Result<CallableDeclarationIdentityShapeV1, CrossConeHirNominalAuthorityError> {
        match declaration {
            CallableTemplateOrigin::Function(_)
            | CallableTemplateOrigin::GenericFunction(_)
            | CallableTemplateOrigin::Constructor(_) => self.function_identity_shape(declaration),
            CallableTemplateOrigin::Accessor(accessor) => self.accessor_identity_shape(accessor),
            CallableTemplateOrigin::VariantConstructor(variant) => {
                self.variant_constructor_identity_shape(variant)
            }
        }
    }
}
