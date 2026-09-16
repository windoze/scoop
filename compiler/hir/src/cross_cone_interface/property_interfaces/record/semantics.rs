use scoop_identity::{
    AccessorRole, NominalDeclarationOwner, PersistentPropertyAccessorId, PropertyAccessorKey,
    SignatureTypeKey,
};

use super::PropertyInterfaceRecordV1;
use crate::{
    NominalInterfaceShapeAuthority, PropertyCapabilityV1, PropertyDeclarationId,
    PropertyPublicAccessV1, PropertyRepresentationV1, PublicDeclarationOwnerV1,
    PublicNominalKindV1, PublicNominalShapeV1,
};

mod errors;

pub use errors::PropertyInterfaceSemanticValidationError;

/// Identity-derived facts for one ordinary or extension property.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyDeclarationIdentityShapeV1 {
    owner: PublicDeclarationOwnerV1,
    own_type_parameter_arity: u32,
    outer_type_parameter_arity: u32,
    receiver: Option<SignatureTypeKey>,
}

impl PropertyDeclarationIdentityShapeV1 {
    pub fn new(
        owner: PublicDeclarationOwnerV1,
        own_type_parameter_arity: u32,
        outer_type_parameter_arity: u32,
        receiver: Option<SignatureTypeKey>,
    ) -> Self {
        Self {
            owner,
            own_type_parameter_arity,
            outer_type_parameter_arity,
            receiver,
        }
    }

    pub const fn owner(&self) -> PublicDeclarationOwnerV1 {
        self.owner
    }

    pub const fn own_type_parameter_arity(&self) -> u32 {
        self.own_type_parameter_arity
    }

    pub const fn outer_type_parameter_arity(&self) -> u32 {
        self.outer_type_parameter_arity
    }

    pub fn receiver(&self) -> Option<&SignatureTypeKey> {
        self.receiver.as_ref()
    }
}

/// Definition-side source facts that cannot be recovered from persistent ids.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PropertyDeclarationSourceShapeV1 {
    capability: PropertyCapabilityV1,
    representation: PropertyRepresentationV1,
    access: PropertyPublicAccessV1,
}

impl PropertyDeclarationSourceShapeV1 {
    pub const fn new(
        capability: PropertyCapabilityV1,
        representation: PropertyRepresentationV1,
        access: PropertyPublicAccessV1,
    ) -> Self {
        Self {
            capability,
            representation,
            access,
        }
    }

    pub const fn capability(self) -> PropertyCapabilityV1 {
        self.capability
    }

    pub const fn representation(self) -> PropertyRepresentationV1 {
        self.representation
    }

    pub const fn access(self) -> PropertyPublicAccessV1 {
        self.access
    }
}

/// Supplies canonical identity, source-HIR, and accessor-key facts.
pub trait PropertyInterfaceSemanticAuthority<E>: NominalInterfaceShapeAuthority<E> {
    fn property_declaration_identity_shape(
        &mut self,
        declaration: PropertyDeclarationId,
    ) -> Result<PropertyDeclarationIdentityShapeV1, E>;

    fn property_declaration_source_shape(
        &mut self,
        declaration: PropertyDeclarationId,
    ) -> Result<PropertyDeclarationSourceShapeV1, E>;

    fn property_accessor_key(
        &mut self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<PropertyAccessorKey, E>;
}

impl PropertyInterfaceRecordV1 {
    pub fn validate_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), PropertyInterfaceSemanticValidationError<E>>
    where
        A: PropertyInterfaceSemanticAuthority<E>,
    {
        let identity = authority
            .property_declaration_identity_shape(self.declaration)
            .map_err(PropertyInterfaceSemanticValidationError::Declaration)?;
        self.validate_identity_shape(&identity)?;

        let outer_arity = (identity.outer_type_parameter_arity != 0)
            .then_some(identity.outer_type_parameter_arity);
        self.type_parameters
            .validate_bound_semantics(outer_arity, authority)
            .map_err(PropertyInterfaceSemanticValidationError::TypeParameters)?;
        let scope = self.type_parameters.signature_scope(outer_arity);
        if let Some(receiver) = &self.receiver {
            scope
                .validate_signature_semantics(receiver, authority)
                .map_err(PropertyInterfaceSemanticValidationError::Receiver)?;
        }
        scope
            .validate_signature_semantics(&self.value_type, authority)
            .map_err(PropertyInterfaceSemanticValidationError::ValueType)?;

        self.validate_accessor(self.capability.getter(), AccessorRole::Getter, authority)?;
        if let Some(setter) = self.capability.setter() {
            self.validate_accessor(setter, AccessorRole::Setter, authority)?;
        }

        let source = authority
            .property_declaration_source_shape(self.declaration)
            .map_err(PropertyInterfaceSemanticValidationError::Source)?;
        self.validate_source_shape(source)?;
        self.validate_const_owner(authority)
    }

    fn validate_identity_shape<E>(
        &self,
        identity: &PropertyDeclarationIdentityShapeV1,
    ) -> Result<(), PropertyInterfaceSemanticValidationError<E>> {
        if self.owner != identity.owner {
            return Err(PropertyInterfaceSemanticValidationError::Owner {
                expected: identity.owner,
                actual: self.owner,
            });
        }
        let actual_arity = self.type_parameters.len_u32();
        if actual_arity != identity.own_type_parameter_arity {
            return Err(
                PropertyInterfaceSemanticValidationError::TypeParameterArity {
                    expected: identity.own_type_parameter_arity,
                    actual: actual_arity,
                },
            );
        }
        if self.receiver.as_ref() != identity.receiver.as_ref() {
            return Err(PropertyInterfaceSemanticValidationError::ReceiverMismatch {
                expected: identity.receiver.clone().map(Box::new),
                actual: self.receiver.clone().map(Box::new),
            });
        }
        Ok(())
    }

    fn validate_accessor<A, E>(
        &self,
        accessor: PersistentPropertyAccessorId,
        expected_role: AccessorRole,
        authority: &mut A,
    ) -> Result<(), PropertyInterfaceSemanticValidationError<E>>
    where
        A: PropertyInterfaceSemanticAuthority<E>,
    {
        let key = authority.property_accessor_key(accessor).map_err(|error| {
            PropertyInterfaceSemanticValidationError::AccessorReference {
                accessor,
                expected_role,
                error,
            }
        })?;
        if key.owner() != self.declaration {
            return Err(PropertyInterfaceSemanticValidationError::AccessorOwner {
                accessor,
                expected: self.declaration,
                actual: key.owner(),
            });
        }
        if key.role() != expected_role {
            return Err(PropertyInterfaceSemanticValidationError::AccessorRole {
                accessor,
                expected: expected_role,
                actual: key.role(),
            });
        }
        Ok(())
    }

    fn validate_source_shape<E>(
        &self,
        source: PropertyDeclarationSourceShapeV1,
    ) -> Result<(), PropertyInterfaceSemanticValidationError<E>> {
        if self.capability != source.capability {
            return Err(PropertyInterfaceSemanticValidationError::Capability {
                expected: Box::new(source.capability),
                actual: Box::new(self.capability),
            });
        }
        if self.representation != source.representation {
            return Err(PropertyInterfaceSemanticValidationError::Representation {
                expected: source.representation,
                actual: self.representation,
            });
        }
        if self.access != source.access {
            return Err(PropertyInterfaceSemanticValidationError::Access {
                expected: source.access,
                actual: self.access,
            });
        }
        Ok(())
    }

    fn validate_const_owner<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), PropertyInterfaceSemanticValidationError<E>>
    where
        A: PropertyInterfaceSemanticAuthority<E>,
    {
        if self.representation != PropertyRepresentationV1::Const {
            return Ok(());
        }
        let PublicDeclarationOwnerV1::Nominal(owner) = self.owner else {
            return Ok(());
        };
        let shape = match owner {
            NominalDeclarationOwner::Concrete(declaration) => authority
                .concrete_nominal_shape(declaration)
                .map_err(PropertyInterfaceSemanticValidationError::ConstOwner)?,
            NominalDeclarationOwner::GenericTemplate(declaration) => authority
                .generic_nominal_shape(declaration)
                .map_err(PropertyInterfaceSemanticValidationError::ConstOwner)?,
        };
        validate_const_nominal_shape(shape)
    }
}

fn validate_const_nominal_shape<E>(
    shape: PublicNominalShapeV1,
) -> Result<(), PropertyInterfaceSemanticValidationError<E>> {
    if shape.kind() != PublicNominalKindV1::Object {
        return Err(PropertyInterfaceSemanticValidationError::ConstOwnerKind {
            actual: shape.kind(),
        });
    }
    if shape.type_parameter_arity() != 0 {
        return Err(PropertyInterfaceSemanticValidationError::ConstOwnerArity {
            actual: shape.type_parameter_arity(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
