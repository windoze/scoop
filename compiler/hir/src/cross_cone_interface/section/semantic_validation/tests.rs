use std::borrow::Cow;
use std::convert::Infallible;

use scoop_identity::{
    BindingTarget, ConeIdentity, EnumVariantFieldKey, EnumVariantIdentityKey, ExportBindingKey,
    FieldIdentityKey, LocalValueSelector, PersistentConstructorId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExportBindingId, PersistentFieldId, PersistentGenericTypeId,
    PersistentObjectValueId, PersistentPropertyAccessorId, PersistentPropertyId,
    PersistentTypeAliasId, PersistentTypeId, PropertyAccessorKey, SignatureTypeKey,
    SourceDeclarationKey, StructuralDefinitionPath,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

use super::*;
use crate::{
    CallableDeclarationId, CallableDeclarationIdentityShapeV1,
    CallableSourceInterfaceSemanticAuthority, CanonicalCallableInterfacesV1,
    CanonicalCallableSourceInterfacesV1, CanonicalExportConstValuesV1,
    CanonicalExportDefaultTemplatesV1, CanonicalExportDefinitionSourcesV1,
    CanonicalExternalHirReferencesV1, CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1,
    CanonicalPublicExportBindingsV1, CanonicalTypeAliasInterfacesV1,
    ConstPropertyDeclarationSourceV1, DefaultConstructorRefV1, DefaultFieldRefV1,
    DefaultLocalDataFlowSemanticAuthority, DefaultNestedCallableAbiShapeV1,
    DefaultNestedCallableBodyArgumentsV1, DefaultNestedCallableIdentityShapeV1,
    DefaultNestedCallableIdentityV1, DefaultNestedCallableSemanticAuthority,
    DefaultOperationCoreTypeV1, DefaultOperationEntityShapeV1, DefaultOperationEntityV1,
    DefaultOperationIntrinsicV1, DefaultOperationTypeRelationV1,
    DefaultOperationTypingSemanticAuthority, DefaultReferenceSemanticAuthority,
    DefaultTemplateOriginSemanticAuthority, DefaultTemplateProviderShapeV1,
    DefaultTemplateRootSemanticAuthority, ExportConstValueSemanticAuthority,
    ExportDefaultCallableTargetV1, ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1,
    ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceV1,
    ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1, NominalInterfaceSemanticAuthority,
    NominalInterfaceShapeAuthority, NominalSourceShapeSemanticAuthority, PropertyDeclarationId,
    PropertyDeclarationIdentityShapeV1, PropertyDeclarationSourceShapeV1,
    PropertyInterfaceRecordV1, PropertyInterfaceSemanticAuthority, PublicDeclarationOwnerV1,
    PublicExportBindingClosureAuthority, PublicMemberRefV1, PublicNominalShapeV1,
    TypeAliasDeclarationSourceV1, TypeAliasInterfaceSemanticAuthority,
};

#[test]
fn complete_validator_accepts_an_empty_interface() {
    assert!(
        empty_section()
            .validate_semantics(
                ConeIdentity::SINGLE_FILE,
                &CanonicalDirectPublicSurfaceV1::try_new(Vec::new()).unwrap(),
                &mut EmptyAuthority(ConeIdentity::SINGLE_FILE),
                &mut BudgetMeter::new(DecodeLimits::default()),
                &WirePath::root(),
            )
            .is_ok()
    );
}

#[test]
fn complete_validator_stops_at_the_first_failed_phase() {
    let fixture = crate::cross_cone_interface::public_bindings::direct_fixture(
        ConeIdentity::SINGLE_FILE,
        "entry",
    );
    let binding = fixture.binding.id();
    let direct_surface = CanonicalDirectPublicSurfaceV1::try_new(vec![binding]).unwrap();

    assert!(matches!(
        empty_section().validate_semantics(
            ConeIdentity::SINGLE_FILE,
            &direct_surface,
            &mut EmptyAuthority(ConeIdentity::SINGLE_FILE),
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        ),
        Err(CrossConeHirInterfaceSemanticValidationError::Internal(error))
            if matches!(
                error.as_ref(),
                CrossConeHirInternalClosureValidationError::DirectSurface(
                    crate::PublicExportBindingDirectSurfaceValidationError::MissingDeclaredCurrent {
                        binding: actual,
                        ..
                    }
                ) if *actual == binding
            )
    ));
}

fn empty_section() -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    )
}

pub(crate) struct EmptyAuthority(pub ConeIdentity);

impl NominalInterfaceShapeAuthority<Infallible> for EmptyAuthority {
    fn concrete_nominal_shape(
        &mut self,
        _declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, Infallible> {
        unreachable!()
    }

    fn generic_nominal_shape(
        &mut self,
        _declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, Infallible> {
        unreachable!()
    }
}

impl NominalSourceShapeSemanticAuthority<Infallible> for EmptyAuthority {
    fn nominal_field_key(
        &mut self,
        _field: PersistentFieldId,
    ) -> Result<Cow<'_, FieldIdentityKey>, Infallible> {
        unreachable!()
    }

    fn enum_variant_key(
        &mut self,
        _variant: PersistentEnumVariantId,
    ) -> Result<Cow<'_, EnumVariantIdentityKey>, Infallible> {
        unreachable!()
    }

    fn enum_variant_field_key(
        &mut self,
        _field: PersistentEnumVariantFieldId,
    ) -> Result<Cow<'_, EnumVariantFieldKey>, Infallible> {
        unreachable!()
    }

    fn object_value_key(
        &mut self,
        _value: PersistentObjectValueId,
    ) -> Result<Cow<'_, SourceDeclarationKey>, Infallible> {
        unreachable!()
    }
}

impl NominalInterfaceSemanticAuthority<Infallible> for EmptyAuthority {
    fn validate_nominal_declaration(
        &mut self,
        declaration: &crate::NominalInterfaceRecordV1,
    ) -> Result<(), Infallible> {
        self.nominal_declaration_key(declaration.declaration())
            .map(|_| ())
    }

    fn nominal_declaration_key(
        &mut self,
        _declaration: crate::SourceNominalId,
    ) -> Result<SourceDeclarationKey, Infallible> {
        unreachable!()
    }

    fn constructor_owner(
        &mut self,
        _constructor: PersistentConstructorId,
    ) -> Result<PublicDeclarationOwnerV1, Infallible> {
        unreachable!()
    }

    fn member_owner(
        &mut self,
        _member: PublicMemberRefV1,
    ) -> Result<PublicDeclarationOwnerV1, Infallible> {
        unreachable!()
    }

    fn nested_binding_owner(
        &mut self,
        _binding: PersistentExportBindingId,
    ) -> Result<PublicDeclarationOwnerV1, Infallible> {
        unreachable!()
    }
}

impl CallableInterfaceSemanticAuthority<Infallible> for EmptyAuthority {
    fn callable_declaration_identity_shape(
        &mut self,
        _declaration: CallableDeclarationId,
    ) -> Result<CallableDeclarationIdentityShapeV1, Infallible> {
        unreachable!()
    }
}

impl PropertyInterfaceSemanticAuthority<Infallible> for EmptyAuthority {
    fn property_declaration_identity_shape(
        &mut self,
        _declaration: PropertyDeclarationId,
    ) -> Result<PropertyDeclarationIdentityShapeV1, Infallible> {
        unreachable!()
    }

    fn property_declaration_source_shape(
        &mut self,
        _declaration: PropertyDeclarationId,
    ) -> Result<PropertyDeclarationSourceShapeV1, Infallible> {
        unreachable!()
    }

    fn property_accessor_key(
        &mut self,
        _accessor: PersistentPropertyAccessorId,
    ) -> Result<PropertyAccessorKey, Infallible> {
        unreachable!()
    }
}

impl TypeAliasInterfaceSemanticAuthority<Infallible> for EmptyAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.0
    }

    fn type_alias_declaration_source(
        &mut self,
        _alias: PersistentTypeAliasId,
    ) -> Result<TypeAliasDeclarationSourceV1, Infallible> {
        unreachable!()
    }
}

impl CallableSourceInterfaceSemanticAuthority<Infallible> for EmptyAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.0
    }

    fn validate_array_type(&mut self, _array: PersistentGenericTypeId) -> Result<(), Infallible> {
        unreachable!()
    }

    fn validate_source_parameter_origin(
        &mut self,
        _owner: CallableDeclarationId,
        _position: u32,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), Infallible> {
        unreachable!()
    }
}

impl ExportDefinitionSourceSemanticAuthority<Infallible> for EmptyAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.0
    }

    fn validate_export_definition_source(
        &mut self,
        _source: &ExportDefinitionSourceV1,
    ) -> Result<(), Infallible> {
        unreachable!()
    }
}

impl DefaultTemplateRootSemanticAuthority<Infallible> for EmptyAuthority {
    fn default_template_provider_shape(
        &mut self,
        _root: crate::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
    ) -> Result<DefaultTemplateProviderShapeV1, Infallible> {
        unreachable!()
    }

    fn default_template_provider_parameter(
        &mut self,
        _root: crate::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
    ) -> Result<crate::DefaultTemplateProviderParameterV1<'_>, Infallible> {
        panic!("empty public interface has no default provider")
    }

    fn default_template_provider_receiver(
        &mut self,
        _root: crate::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
    ) -> Result<Option<SignatureTypeKey>, Infallible> {
        panic!("empty public interface has no default provider")
    }

    fn validate_inherited_default_provider(
        &mut self,
        _key: ExportDefaultTemplateKeyV1,
        _root: crate::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _mapping: &crate::CanonicalBinderUseListV1,
    ) -> Result<(), Infallible> {
        unreachable!()
    }
}

impl DefaultTemplateOriginSemanticAuthority<Infallible> for EmptyAuthority {
    fn validate_default_template_origin(
        &mut self,
        _key: ExportDefaultTemplateKeyV1,
        _root: crate::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), Infallible> {
        unreachable!()
    }

    fn validate_default_template_local_origin(
        &mut self,
        _key: ExportDefaultTemplateKeyV1,
        _root: crate::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _selector: &LocalValueSelector,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), Infallible> {
        unreachable!()
    }
}

impl DefaultLocalDataFlowSemanticAuthority<Infallible> for EmptyAuthority {
    fn default_binding_struct_field_index(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _declaration: PersistentFieldId,
        _owner_type: &SignatureTypeKey,
    ) -> Result<u32, Infallible> {
        unreachable!()
    }
}

impl DefaultOperationTypingSemanticAuthority<Infallible> for EmptyAuthority {
    fn canonical_default_operation_type(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _role: DefaultOperationCoreTypeV1,
    ) -> Result<SignatureTypeKey, Infallible> {
        unreachable!()
    }

    fn classify_default_core_application(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _value: &SignatureTypeKey,
    ) -> Result<Option<crate::DefaultCoreApplicationV1>, Infallible> {
        unreachable!()
    }

    fn default_operation_entity_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _entity: DefaultOperationEntityV1<'_>,
    ) -> Result<DefaultOperationEntityShapeV1, Infallible> {
        unreachable!()
    }

    fn default_operation_type_relation(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _relation: DefaultOperationTypeRelationV1,
        _source: &SignatureTypeKey,
        _target: &SignatureTypeKey,
    ) -> Result<bool, Infallible> {
        unreachable!()
    }

    fn validate_default_operation_intrinsic(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _intrinsic: DefaultOperationIntrinsicV1<'_>,
    ) -> Result<(), Infallible> {
        unreachable!()
    }
}

impl DefaultNestedCallableSemanticAuthority<Infallible> for EmptyAuthority {
    fn default_nested_callable_identity_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _identity: DefaultNestedCallableIdentityV1,
        _site: crate::DefaultNestedCallableSiteV1,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, Infallible> {
        unreachable!()
    }

    fn default_nested_callable_abi_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _identity: DefaultNestedCallableIdentityV1,
        _site: crate::DefaultNestedCallableSiteV1,
        _body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
    ) -> Result<DefaultNestedCallableAbiShapeV1, Infallible> {
        unreachable!()
    }
}

impl DefaultReferenceSemanticAuthority<Infallible> for EmptyAuthority {
    fn validate_default_callable_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &ExportDefaultCallableTargetV1,
    ) -> Result<(), Infallible> {
        unreachable!()
    }

    fn validate_default_constructor_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &DefaultConstructorRefV1,
    ) -> Result<(), Infallible> {
        unreachable!()
    }

    fn validate_default_type_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &SignatureTypeKey,
    ) -> Result<(), Infallible> {
        unreachable!()
    }

    fn validate_default_global_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: PersistentPropertyId,
    ) -> Result<(), Infallible> {
        unreachable!()
    }

    fn validate_default_singleton_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: PersistentObjectValueId,
    ) -> Result<(), Infallible> {
        unreachable!()
    }

    fn validate_default_field_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &DefaultFieldRefV1,
    ) -> Result<(), Infallible> {
        unreachable!()
    }
}

impl ExportConstValueSemanticAuthority<Infallible> for EmptyAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.0
    }

    fn const_property_declaration_source(
        &mut self,
        _property: PersistentPropertyId,
    ) -> Result<ConstPropertyDeclarationSourceV1, Infallible> {
        unreachable!()
    }

    fn validated_property_interface(
        &mut self,
        _property: PersistentPropertyId,
    ) -> Result<&PropertyInterfaceRecordV1, Infallible> {
        unreachable!()
    }

    fn validate_const_value_type(
        &mut self,
        _value_type: PersistentTypeId,
        _kind: crate::CanonicalConstValueKindV1,
    ) -> Result<(), Infallible> {
        unreachable!()
    }
}

impl PublicExportBindingClosureAuthority for EmptyAuthority {
    fn closure_node_count(&self) -> usize {
        1
    }

    fn is_direct_dependency(&self, _provider: ConeIdentity) -> bool {
        false
    }

    fn binding_key(&self, _binding: PersistentExportBindingId) -> Option<&ExportBindingKey> {
        None
    }

    fn public_bindings(&self, _exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1> {
        None
    }
}

impl ExternalHirReferenceSemanticAuthority<Infallible> for EmptyAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.0
    }

    fn external_hir_target_origin(
        &mut self,
        _target: ExternalHirTargetV1,
    ) -> Result<ConeIdentity, Infallible> {
        unreachable!()
    }

    fn external_hir_target_binding_root(
        &mut self,
        _target: ExternalHirTargetV1,
    ) -> Result<BindingTarget, Infallible> {
        unreachable!()
    }
}

impl crate::DefaultLocalFunctionSignatureAuthority<Infallible> for EmptyAuthority {
    fn default_local_function_own_binder_arity(
        &mut self,
        _declaration: scoop_identity::CallableTemplateOrigin,
        _meter: &mut scoop_wire::BudgetMeter,
        _path: &scoop_wire::WirePath,
    ) -> Result<u32, Infallible> {
        panic!("empty fixture has no local function")
    }
}
