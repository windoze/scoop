use std::collections::BTreeMap;

use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, ConeIdentity, DeclarationScope,
    DefinitionOwnerAtom, DefinitionOwnerChain, EnumVariantFieldKey, EnumVariantIdentityKey,
    FieldIdentityKey, NonEmptyVec, PackagePath, PersistentConstructorId,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentExportBindingId,
    PersistentFieldId, PersistentGenericTypeId, PersistentObjectValueId, PersistentTypeId,
    SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

use super::super::*;
use crate::{
    CanonicalBinderListV1, CanonicalPersistentIdsV1, CanonicalPublicMemberRefsV1,
    CanonicalSignatureTypesV1, NominalInterfaceShapeAuthority, NominalSourceShapeSemanticAuthority,
    NominalSourceShapeV1, PublicMemberRefV1, PublicNominalShapeV1, StructSourceFieldV1,
    StructSourceShapeV1, TypeParameterBinderV1, TypeParameterBoundsV1,
};

pub(super) struct Fixture {
    pub(super) owner: SourceNominalId,
    owner_key: SourceDeclarationKey,
    pub(super) struct_owner: SourceNominalId,
    struct_key: SourceDeclarationKey,
    pub(super) base_class: PersistentTypeId,
    pub(super) second_base_class: PersistentTypeId,
    pub(super) generic_interface: PersistentGenericTypeId,
    pub(super) value_struct: PersistentTypeId,
    pub(super) constructor: PersistentConstructorId,
    pub(super) member: PublicMemberRefV1,
    pub(super) nested_binding: PersistentExportBindingId,
    struct_field: PersistentFieldId,
    foreign_struct_field_key: FieldIdentityKey,
    shapes: BTreeMap<SourceNominalId, PublicNominalShapeV1>,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let owner_key = nominal("Owner", SourceNominalKind::Class, 1);
        let owner_id = PersistentGenericTypeId::from_source_declaration(&owner_key).unwrap();
        let owner = SourceNominalId::GenericTemplate(owner_id);
        let struct_key = nominal("Value", SourceNominalKind::Struct, 0);
        let struct_owner = source_owner(&struct_key);
        let foreign_struct = nominal("ForeignValue", SourceNominalKind::Struct, 0);

        let base_class_key = nominal("Base", SourceNominalKind::Class, 0);
        let base_class = PersistentTypeId::from_source_declaration(&base_class_key).unwrap();
        let second_base_key = nominal("SecondBase", SourceNominalKind::Class, 0);
        let second_base_class =
            PersistentTypeId::from_source_declaration(&second_base_key).unwrap();
        let generic_interface_key = nominal("Contract", SourceNominalKind::Interface, 1);
        let generic_interface =
            PersistentGenericTypeId::from_source_declaration(&generic_interface_key).unwrap();
        let value_struct_key = nominal("Payload", SourceNominalKind::Struct, 0);
        let value_struct = PersistentTypeId::from_source_declaration(&value_struct_key).unwrap();

        let member_site = owned_site(DefinitionOwnerAtom::GenericType(owner_id));
        let constructor_key = SourceDeclarationKey::constructor(member_site.clone(), Vec::new());
        let constructor =
            PersistentConstructorId::from_source_declaration(&constructor_key).unwrap();
        let member_key =
            SourceDeclarationKey::function(member_site, identifier("run"), 0, None, Vec::new());
        let member = PublicMemberRefV1::Callable(CallableTemplateOrigin::Function(
            scoop_identity::PersistentFunctionId::from_source_declaration(&member_key).unwrap(),
        ));

        let nested_key = SourceDeclarationKey::nominal(
            owned_site(DefinitionOwnerAtom::GenericType(owner_id)),
            identifier("Nested"),
            SourceNominalKind::Class,
            0,
        );
        let nested_target = scoop_identity::BindingTarget::type_name(&nested_key).unwrap();
        let nested_binding_key = scoop_identity::ExportBindingKey::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            identifier("Nested"),
            nested_target,
        );
        let nested_binding = PersistentExportBindingId::from_key(&nested_binding_key).unwrap();

        let foreign_struct_field_key =
            FieldIdentityKey::source_declared(&foreign_struct, identifier("value")).unwrap();
        let struct_field = PersistentFieldId::from_key(&foreign_struct_field_key).unwrap();

        Self {
            owner,
            owner_key,
            struct_owner,
            struct_key,
            base_class,
            second_base_class,
            generic_interface,
            value_struct,
            constructor,
            member,
            nested_binding,
            struct_field,
            foreign_struct_field_key,
            shapes: BTreeMap::from([
                (
                    SourceNominalId::Concrete(base_class),
                    PublicNominalShapeV1::new(PublicNominalKindV1::Class, 0),
                ),
                (
                    SourceNominalId::Concrete(second_base_class),
                    PublicNominalShapeV1::new(PublicNominalKindV1::Class, 0),
                ),
                (
                    SourceNominalId::GenericTemplate(generic_interface),
                    PublicNominalShapeV1::new(PublicNominalKindV1::Interface, 1),
                ),
                (
                    SourceNominalId::Concrete(value_struct),
                    PublicNominalShapeV1::new(PublicNominalKindV1::Struct, 0),
                ),
            ]),
        }
    }

    pub(super) fn record(&self) -> NominalInterfaceRecordV1 {
        self.record_with(
            PublicNominalKindV1::Class,
            binders(),
            supertypes(vec![
                SignatureTypeKey::Nominal(self.base_class),
                generic_application(self.generic_interface, binder(0)),
            ]),
            NominalSourceShapeV1::Class,
        )
    }

    pub(super) fn record_with(
        &self,
        kind: PublicNominalKindV1,
        type_parameters: CanonicalBinderListV1,
        exact_supertypes: CanonicalSignatureTypesV1,
        source_shape: NominalSourceShapeV1,
    ) -> NominalInterfaceRecordV1 {
        NominalInterfaceRecordV1::try_new(
            self.owner,
            kind,
            type_parameters,
            exact_supertypes,
            CanonicalPersistentIdsV1::try_new(vec![self.constructor]).unwrap(),
            CanonicalPublicMemberRefsV1::try_new(vec![self.member]).unwrap(),
            CanonicalPersistentIdsV1::try_new(vec![self.nested_binding]).unwrap(),
            source_shape,
        )
        .unwrap()
    }

    pub(super) fn struct_record(
        &self,
        exact_supertypes: CanonicalSignatureTypesV1,
    ) -> NominalInterfaceRecordV1 {
        NominalInterfaceRecordV1::try_new(
            self.struct_owner,
            PublicNominalKindV1::Struct,
            empty_binders(),
            exact_supertypes,
            CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
            CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
            CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
            NominalSourceShapeV1::Struct(
                StructSourceShapeV1::try_new(vec![StructSourceFieldV1::new(
                    self.struct_field,
                    SignatureTypeKey::Nominal(self.base_class),
                )])
                .unwrap(),
            ),
        )
        .unwrap()
    }

    pub(super) fn authority(&self) -> TestAuthority {
        TestAuthority {
            nominals: BTreeMap::from([
                (self.owner, self.owner_key.clone()),
                (self.struct_owner, self.struct_key.clone()),
            ]),
            shapes: self.shapes.clone(),
            constructor_owner: PublicDeclarationOwnerV1::Nominal(self.owner),
            constructor: self.constructor,
            member_owner: PublicDeclarationOwnerV1::Nominal(self.owner),
            member: self.member,
            nested_binding_owner: PublicDeclarationOwnerV1::Nominal(self.owner),
            nested_binding: self.nested_binding,
            fields: BTreeMap::from([(self.struct_field, self.foreign_struct_field_key.clone())]),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestAuthorityError {
    Concrete(PersistentTypeId),
    Generic(PersistentGenericTypeId),
    Nominal(SourceNominalId),
    Constructor(PersistentConstructorId),
    Member(PublicMemberRefV1),
    Binding(PersistentExportBindingId),
    Field(PersistentFieldId),
    Variant(PersistentEnumVariantId),
    VariantField(PersistentEnumVariantFieldId),
    ObjectValue(PersistentObjectValueId),
}

impl std::fmt::Display for TestAuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing test authority: {self:?}")
    }
}

impl std::error::Error for TestAuthorityError {}

pub(super) struct TestAuthority {
    pub(super) nominals: BTreeMap<SourceNominalId, SourceDeclarationKey>,
    shapes: BTreeMap<SourceNominalId, PublicNominalShapeV1>,
    pub(super) constructor_owner: PublicDeclarationOwnerV1,
    constructor: PersistentConstructorId,
    pub(super) member_owner: PublicDeclarationOwnerV1,
    member: PublicMemberRefV1,
    pub(super) nested_binding_owner: PublicDeclarationOwnerV1,
    nested_binding: PersistentExportBindingId,
    fields: BTreeMap<PersistentFieldId, FieldIdentityKey>,
}

impl NominalInterfaceShapeAuthority<TestAuthorityError> for TestAuthority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        self.shapes
            .get(&SourceNominalId::Concrete(declaration))
            .copied()
            .ok_or(TestAuthorityError::Concrete(declaration))
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        self.shapes
            .get(&SourceNominalId::GenericTemplate(declaration))
            .copied()
            .ok_or(TestAuthorityError::Generic(declaration))
    }
}

impl NominalSourceShapeSemanticAuthority<TestAuthorityError> for TestAuthority {
    fn struct_field_key(
        &mut self,
        field: PersistentFieldId,
    ) -> Result<FieldIdentityKey, TestAuthorityError> {
        self.fields
            .get(&field)
            .cloned()
            .ok_or(TestAuthorityError::Field(field))
    }

    fn enum_variant_key(
        &mut self,
        variant: PersistentEnumVariantId,
    ) -> Result<EnumVariantIdentityKey, TestAuthorityError> {
        Err(TestAuthorityError::Variant(variant))
    }

    fn enum_variant_field_key(
        &mut self,
        field: PersistentEnumVariantFieldId,
    ) -> Result<EnumVariantFieldKey, TestAuthorityError> {
        Err(TestAuthorityError::VariantField(field))
    }

    fn object_value_key(
        &mut self,
        value: PersistentObjectValueId,
    ) -> Result<SourceDeclarationKey, TestAuthorityError> {
        Err(TestAuthorityError::ObjectValue(value))
    }
}

impl NominalInterfaceSemanticAuthority<TestAuthorityError> for TestAuthority {
    fn nominal_declaration_key(
        &mut self,
        declaration: SourceNominalId,
    ) -> Result<SourceDeclarationKey, TestAuthorityError> {
        self.nominals
            .get(&declaration)
            .cloned()
            .ok_or(TestAuthorityError::Nominal(declaration))
    }

    fn constructor_owner(
        &mut self,
        constructor: PersistentConstructorId,
    ) -> Result<PublicDeclarationOwnerV1, TestAuthorityError> {
        if constructor == self.constructor {
            Ok(self.constructor_owner)
        } else {
            Err(TestAuthorityError::Constructor(constructor))
        }
    }

    fn member_owner(
        &mut self,
        member: PublicMemberRefV1,
    ) -> Result<PublicDeclarationOwnerV1, TestAuthorityError> {
        if member == self.member {
            Ok(self.member_owner)
        } else {
            Err(TestAuthorityError::Member(member))
        }
    }

    fn nested_binding_owner(
        &mut self,
        binding: PersistentExportBindingId,
    ) -> Result<PublicDeclarationOwnerV1, TestAuthorityError> {
        if binding == self.nested_binding {
            Ok(self.nested_binding_owner)
        } else {
            Err(TestAuthorityError::Binding(binding))
        }
    }
}

pub(super) fn binders() -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
        identifier("T"),
        TypeParameterBoundsV1::Unconstrained,
    )])
    .unwrap()
}

pub(super) fn empty_binders() -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(Vec::new()).unwrap()
}

pub(super) fn class_bound_binders(
    class: PersistentGenericTypeId,
    argument: SignatureTypeKey,
) -> CanonicalBinderListV1 {
    let bounds = crate::NominalTypeParameterBoundsV1::try_new(
        Some(generic_application(class, argument)),
        CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
    )
    .unwrap();
    CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
        identifier("T"),
        TypeParameterBoundsV1::Nominal(bounds),
    )])
    .unwrap()
}

pub(super) fn empty_supertypes() -> CanonicalSignatureTypesV1 {
    supertypes(Vec::new())
}

pub(super) fn generic_object_record() -> (
    NominalInterfaceRecordV1,
    SourceNominalId,
    SourceDeclarationKey,
) {
    let key = nominal("GenericObject", SourceNominalKind::Object, 1);
    let declaration = SourceNominalId::GenericTemplate(
        PersistentGenericTypeId::from_source_declaration(&key).unwrap(),
    );
    let value_key = nominal("ObjectValue", SourceNominalKind::Object, 0);
    let value = PersistentObjectValueId::from_source_object(&value_key).unwrap();
    let record = NominalInterfaceRecordV1::try_new(
        declaration,
        PublicNominalKindV1::Object,
        binders(),
        empty_supertypes(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        NominalSourceShapeV1::Object(crate::ObjectSourceShapeV1::new(value)),
    )
    .unwrap();
    (record, declaration, key)
}

pub(super) fn supertypes(values: Vec<SignatureTypeKey>) -> CanonicalSignatureTypesV1 {
    CanonicalSignatureTypesV1::try_new(values).unwrap()
}

pub(super) fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

fn generic_application(
    origin: PersistentGenericTypeId,
    argument: SignatureTypeKey,
) -> SignatureTypeKey {
    SignatureTypeKey::NominalApplication {
        origin,
        arguments: NonEmptyVec::new(vec![argument]).unwrap(),
    }
}

fn source_owner(key: &SourceDeclarationKey) -> SourceNominalId {
    SourceNominalId::from_source_declaration(key).unwrap()
}

fn nominal(name: &str, kind: SourceNominalKind, type_parameter_count: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        identifier(name),
        kind,
        type_parameter_count,
    )
}

fn owned_site(owner: DefinitionOwnerAtom) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(vec![owner]),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
