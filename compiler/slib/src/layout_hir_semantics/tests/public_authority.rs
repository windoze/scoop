use scoop_hir::*;
use scoop_identity::*;
use std::borrow::Cow;

use super::type_authority::TestAuthorityError;
use crate::{LayoutHirPublicAuthorityContextV1, LayoutHirPublicAuthorityFactoryV1};

pub(super) struct RecordingPublicFactory {
    expected: ConeIdentity,
    build_count: usize,
    dependency_providers: Vec<ConeIdentity>,
    dependency_proof_addresses: Vec<usize>,
}

impl RecordingPublicFactory {
    pub(super) const fn new(expected: ConeIdentity) -> Self {
        Self {
            expected,
            build_count: 0,
            dependency_providers: Vec::new(),
            dependency_proof_addresses: Vec::new(),
        }
    }

    pub(super) const fn build_count(&self) -> usize {
        self.build_count
    }

    pub(super) fn dependency_providers(&self) -> &[ConeIdentity] {
        &self.dependency_providers
    }

    pub(super) fn dependency_proof_addresses(&self) -> &[usize] {
        &self.dependency_proof_addresses
    }
}

impl LayoutHirPublicAuthorityFactoryV1 for RecordingPublicFactory {
    type Error = TestAuthorityError;
    type Authority<'a> = EmptyPublicAuthority;

    fn build<'a>(
        &'a mut self,
        context: LayoutHirPublicAuthorityContextV1<'a>,
    ) -> Result<Self::Authority<'a>, Self::Error> {
        if context.provider() != self.expected {
            return Err(TestAuthorityError::WrongProvider {
                expected: self.expected,
                actual: context.provider(),
            });
        }
        self.build_count += 1;
        self.dependency_providers = context
            .dependencies()
            .iter()
            .map(|dependency| dependency.provider())
            .collect();
        self.dependency_proof_addresses = context
            .dependencies()
            .iter()
            .map(|dependency| dependency.checked().section() as *const _ as usize)
            .collect();
        Ok(EmptyPublicAuthority(context.provider()))
    }
}

pub(super) struct EmptyPublicAuthority(pub ConeIdentity);

impl NominalInterfaceShapeAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn concrete_nominal_shape(
        &mut self,
        _declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn generic_nominal_shape(
        &mut self,
        _declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl NominalSourceShapeSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn nominal_field_key(
        &mut self,
        _field: PersistentFieldId,
    ) -> Result<Cow<'_, FieldIdentityKey>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn enum_variant_key(
        &mut self,
        _variant: PersistentEnumVariantId,
    ) -> Result<Cow<'_, EnumVariantIdentityKey>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn enum_variant_field_key(
        &mut self,
        _field: PersistentEnumVariantFieldId,
    ) -> Result<Cow<'_, EnumVariantFieldKey>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn object_value_key(
        &mut self,
        _value: PersistentObjectValueId,
    ) -> Result<Cow<'_, SourceDeclarationKey>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl NominalInterfaceSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn nominal_declaration_key(
        &mut self,
        _declaration: scoop_hir::SourceNominalId,
    ) -> Result<SourceDeclarationKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn constructor_owner(
        &mut self,
        _constructor: PersistentConstructorId,
    ) -> Result<PublicDeclarationOwnerV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn member_owner(
        &mut self,
        _member: PublicMemberRefV1,
    ) -> Result<PublicDeclarationOwnerV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn nested_binding_owner(
        &mut self,
        _binding: PersistentExportBindingId,
    ) -> Result<PublicDeclarationOwnerV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl CallableInterfaceSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn callable_declaration_identity_shape(
        &mut self,
        _declaration: CallableDeclarationId,
    ) -> Result<CallableDeclarationIdentityShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl PropertyInterfaceSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn property_declaration_identity_shape(
        &mut self,
        _declaration: PropertyDeclarationId,
    ) -> Result<PropertyDeclarationIdentityShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn property_declaration_source_shape(
        &mut self,
        _declaration: PropertyDeclarationId,
    ) -> Result<PropertyDeclarationSourceShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn property_accessor_key(
        &mut self,
        _accessor: PersistentPropertyAccessorId,
    ) -> Result<PropertyAccessorKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl TypeAliasInterfaceSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.0
    }

    fn type_alias_declaration_source(
        &mut self,
        _alias: PersistentTypeAliasId,
    ) -> Result<TypeAliasDeclarationSourceV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl CallableSourceInterfaceSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.0
    }

    fn validate_array_type(
        &mut self,
        _array: PersistentGenericTypeId,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_source_parameter_origin(
        &mut self,
        _owner: CallableDeclarationId,
        _position: u32,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl ExportDefinitionSourceSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.0
    }

    fn validate_export_definition_source(
        &mut self,
        _source: &ExportDefinitionSourceV1,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

mod defaults;
mod routes;
