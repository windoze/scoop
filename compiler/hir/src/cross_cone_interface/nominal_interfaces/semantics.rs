use scoop_identity::{PersistentConstructorId, PersistentExportBindingId, SourceDeclarationKey};

use super::NominalInterfaceRecordV1;
use crate::{
    NominalSourceShapeSemanticAuthority, PublicDeclarationOwnerV1, PublicMemberRefV1,
    PublicNominalKindV1, SourceNominalId,
};

mod dispatch;
mod enclosing;
mod errors;
mod supertypes;

pub use errors::{ExactSupertypeSemanticError, NominalInterfaceSemanticValidationError};

/// Supplies canonical declaration and ownership facts for a resolved nominal
/// interface. Every returned fact must be derived from the canonical key of
/// the exact typed id passed to the method.
pub trait NominalInterfaceSemanticAuthority<E>: NominalSourceShapeSemanticAuthority<E> {
    fn validate_nominal_declaration(
        &mut self,
        declaration: &NominalInterfaceRecordV1,
    ) -> Result<(), E>;

    fn nominal_declaration_key(
        &mut self,
        declaration: SourceNominalId,
    ) -> Result<SourceDeclarationKey, E>;

    fn integer_kind(
        &mut self,
        declaration: SourceNominalId,
    ) -> Result<Option<crate::IntegerKind>, E>;

    fn constructor_owner(
        &mut self,
        constructor: PersistentConstructorId,
    ) -> Result<PublicDeclarationOwnerV1, E>;

    fn member_owner(&mut self, member: PublicMemberRefV1) -> Result<PublicDeclarationOwnerV1, E>;

    fn nested_binding_owner(
        &mut self,
        binding: PersistentExportBindingId,
    ) -> Result<PublicDeclarationOwnerV1, E>;
}

impl NominalInterfaceRecordV1 {
    pub fn validate_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), NominalInterfaceSemanticValidationError<E>>
    where
        A: NominalInterfaceSemanticAuthority<E>,
    {
        let declaration = self.validate_declaration(authority)?;
        authority
            .validate_nominal_declaration(self)
            .map_err(NominalInterfaceSemanticValidationError::Declaration)?;
        self.type_parameters
            .validate_bound_semantics(None, authority)
            .map_err(NominalInterfaceSemanticValidationError::TypeParameters)?;
        self.validate_exact_supertypes(authority)?;
        self.validate_dispatch_applications(authority)?;
        self.validate_owned_entries(authority)?;
        self.source_shape
            .validate_semantics(
                self.declaration,
                self.kind,
                &self.type_parameters,
                authority,
            )
            .map_err(NominalInterfaceSemanticValidationError::SourceShape)?;
        self.validate_enclosing_instance(&declaration, authority)
    }

    fn validate_declaration<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<SourceDeclarationKey, NominalInterfaceSemanticValidationError<E>>
    where
        A: NominalInterfaceSemanticAuthority<E>,
    {
        let key = authority
            .nominal_declaration_key(self.declaration)
            .map_err(NominalInterfaceSemanticValidationError::Declaration)?;
        let actual_kind = key.declaration_kind();
        if actual_kind != self.kind.source_kind() {
            return Err(NominalInterfaceSemanticValidationError::DeclarationKind {
                expected: self.kind,
                actual: actual_kind,
            });
        }
        let expected = key.duplicate_signature().type_parameter_count();
        let actual = self.type_parameters.len_u32();
        if actual != expected {
            return Err(
                NominalInterfaceSemanticValidationError::TypeParameterArity { expected, actual },
            );
        }
        if self.kind == PublicNominalKindV1::Object
            && actual != 0
            && !matches!(&self.source_shape, crate::NominalSourceShapeV1::Object(shape) if shape.object_kind() == crate::ObjectSourceKindV1::Companion)
        {
            return Err(NominalInterfaceSemanticValidationError::ObjectTypeParameters { actual });
        }
        Ok(key)
    }

    fn validate_owned_entries<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), NominalInterfaceSemanticValidationError<E>>
    where
        A: NominalInterfaceSemanticAuthority<E>,
    {
        let expected = PublicDeclarationOwnerV1::Nominal(self.declaration);
        for (index, constructor) in self.constructors.values().iter().copied().enumerate() {
            let actual = authority.constructor_owner(constructor).map_err(|error| {
                NominalInterfaceSemanticValidationError::ConstructorReference { index, error }
            })?;
            if actual != expected {
                return Err(NominalInterfaceSemanticValidationError::ConstructorOwner {
                    index,
                    constructor,
                    expected: self.declaration,
                    actual,
                });
            }
        }
        for (index, member) in self.members.members().iter().copied().enumerate() {
            let actual = authority.member_owner(member).map_err(|error| {
                NominalInterfaceSemanticValidationError::MemberReference { index, error }
            })?;
            if actual != expected {
                return Err(NominalInterfaceSemanticValidationError::MemberOwner {
                    index,
                    member,
                    expected: self.declaration,
                    actual,
                });
            }
        }
        for (index, binding) in self.nested_bindings.values().iter().copied().enumerate() {
            let actual = authority.nested_binding_owner(binding).map_err(|error| {
                NominalInterfaceSemanticValidationError::NestedBindingReference { index, error }
            })?;
            if actual != expected {
                return Err(
                    NominalInterfaceSemanticValidationError::NestedBindingOwner {
                        index,
                        binding,
                        expected: self.declaration,
                        actual,
                    },
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
