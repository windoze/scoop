use std::collections::BTreeMap;

use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, EnumVariantFieldKey,
    EnumVariantFieldSelector, EnumVariantIdentityKey, FieldIdentityKey, NominalDeclarationOwner,
    PackagePath, PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId,
    PersistentGenericTypeId, PersistentObjectValueId, PersistentTypeId, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

use super::super::*;
use crate::{
    CanonicalBinderListV1, EnumSourceShapeV1, NominalInterfaceShapeAuthority, PublicNominalShapeV1,
    TypeParameterBinderV1, TypeParameterBoundsV1,
};

pub(super) struct Fixture {
    pub(super) struct_owner: SourceNominalId,
    pub(super) foreign_struct_owner: SourceNominalId,
    pub(super) struct_field: PersistentFieldId,
    pub(super) foreign_struct_field: PersistentFieldId,
    pub(super) enum_owner: SourceNominalId,
    pub(super) foreign_enum_owner: SourceNominalId,
    pub(super) positional_variant: PersistentEnumVariantId,
    pub(super) named_variant: PersistentEnumVariantId,
    pub(super) constructor_variant: PersistentEnumVariantId,
    pub(super) unit_variant: PersistentEnumVariantId,
    pub(super) foreign_variant: PersistentEnumVariantId,
    pub(super) positional_field: PersistentEnumVariantFieldId,
    pub(super) positional_field_one: PersistentEnumVariantFieldId,
    pub(super) named_field: PersistentEnumVariantFieldId,
    pub(super) constructor_field: PersistentEnumVariantFieldId,
    pub(super) foreign_variant_field: PersistentEnumVariantFieldId,
    pub(super) object_owner: SourceNominalId,
    pub(super) foreign_object_owner: SourceNominalId,
    pub(super) object_value: PersistentObjectValueId,
    pub(super) foreign_object_value: PersistentObjectValueId,
    fields: BTreeMap<PersistentFieldId, FieldIdentityKey>,
    variants: BTreeMap<PersistentEnumVariantId, EnumVariantIdentityKey>,
    variant_fields: BTreeMap<PersistentEnumVariantFieldId, EnumVariantFieldKey>,
    object_values: BTreeMap<PersistentObjectValueId, SourceDeclarationKey>,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let structure = nominal("LocalStruct", SourceNominalKind::Struct, 1);
        let foreign_structure = nominal("ForeignStruct", SourceNominalKind::Struct, 1);
        let enumeration = nominal("LocalEnum", SourceNominalKind::Enum, 1);
        let foreign_enumeration = nominal("ForeignEnum", SourceNominalKind::Enum, 1);
        let object = nominal("LocalObject", SourceNominalKind::Object, 0);
        let foreign_object = nominal("ForeignObject", SourceNominalKind::Object, 0);

        let (struct_field, struct_field_key) = source_field(&structure, "value");
        let (foreign_struct_field, foreign_struct_field_key) =
            source_field(&foreign_structure, "value");
        let (positional_variant, positional_variant_key) =
            source_variant(&enumeration, "Positional");
        let (named_variant, named_variant_key) = source_variant(&enumeration, "Named");
        let (constructor_variant, constructor_variant_key) =
            source_variant(&enumeration, "Constructor");
        let (unit_variant, unit_variant_key) = source_variant(&enumeration, "Unit");
        let (foreign_variant, foreign_variant_key) =
            source_variant(&foreign_enumeration, "Foreign");
        let (positional_field, positional_field_key) = variant_field(
            positional_variant,
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            },
        );
        let (positional_field_one, positional_field_one_key) = variant_field(
            positional_variant,
            EnumVariantFieldSelector::Positional {
                declaration_index: 1,
            },
        );
        let (named_field, named_field_key) = variant_field(
            named_variant,
            EnumVariantFieldSelector::Named(identifier("named")),
        );
        let (constructor_field, constructor_field_key) = variant_field(
            constructor_variant,
            EnumVariantFieldSelector::Named(identifier("argument")),
        );
        let (foreign_variant_field, foreign_variant_field_key) = variant_field(
            foreign_variant,
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            },
        );
        let object_value = PersistentObjectValueId::from_source_object(&object).unwrap();
        let foreign_object_value =
            PersistentObjectValueId::from_source_object(&foreign_object).unwrap();

        Self {
            struct_owner: NominalDeclarationOwner::from_source_declaration(&structure).unwrap(),
            foreign_struct_owner: NominalDeclarationOwner::from_source_declaration(
                &foreign_structure,
            )
            .unwrap(),
            struct_field,
            foreign_struct_field,
            enum_owner: NominalDeclarationOwner::from_source_declaration(&enumeration).unwrap(),
            foreign_enum_owner: NominalDeclarationOwner::from_source_declaration(
                &foreign_enumeration,
            )
            .unwrap(),
            positional_variant,
            named_variant,
            constructor_variant,
            unit_variant,
            foreign_variant,
            positional_field,
            positional_field_one,
            named_field,
            constructor_field,
            foreign_variant_field,
            object_owner: NominalDeclarationOwner::from_source_declaration(&object).unwrap(),
            foreign_object_owner: NominalDeclarationOwner::from_source_declaration(&foreign_object)
                .unwrap(),
            object_value,
            foreign_object_value,
            fields: BTreeMap::from([
                (struct_field, struct_field_key),
                (foreign_struct_field, foreign_struct_field_key),
            ]),
            variants: BTreeMap::from([
                (positional_variant, positional_variant_key),
                (named_variant, named_variant_key),
                (constructor_variant, constructor_variant_key),
                (unit_variant, unit_variant_key),
                (foreign_variant, foreign_variant_key),
            ]),
            variant_fields: BTreeMap::from([
                (positional_field, positional_field_key),
                (positional_field_one, positional_field_one_key),
                (named_field, named_field_key),
                (constructor_field, constructor_field_key),
                (foreign_variant_field, foreign_variant_field_key),
            ]),
            object_values: BTreeMap::from([
                (object_value, object),
                (foreign_object_value, foreign_object),
            ]),
        }
    }

    pub(super) fn authority(&self) -> TestAuthority {
        TestAuthority {
            concrete: BTreeMap::new(),
            generic: BTreeMap::new(),
            fields: self.fields.clone(),
            variants: self.variants.clone(),
            variant_fields: self.variant_fields.clone(),
            object_values: self.object_values.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestAuthorityError {
    Concrete(PersistentTypeId),
    Generic(PersistentGenericTypeId),
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
    concrete: BTreeMap<PersistentTypeId, PublicNominalShapeV1>,
    generic: BTreeMap<PersistentGenericTypeId, PublicNominalShapeV1>,
    pub(super) fields: BTreeMap<PersistentFieldId, FieldIdentityKey>,
    variants: BTreeMap<PersistentEnumVariantId, EnumVariantIdentityKey>,
    variant_fields: BTreeMap<PersistentEnumVariantFieldId, EnumVariantFieldKey>,
    object_values: BTreeMap<PersistentObjectValueId, SourceDeclarationKey>,
}

impl NominalInterfaceShapeAuthority<TestAuthorityError> for TestAuthority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        self.concrete
            .get(&declaration)
            .copied()
            .ok_or(TestAuthorityError::Concrete(declaration))
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        self.generic
            .get(&declaration)
            .copied()
            .ok_or(TestAuthorityError::Generic(declaration))
    }
}

impl NominalSourceShapeSemanticAuthority<TestAuthorityError> for TestAuthority {
    fn struct_field_key(
        &mut self,
        field: PersistentFieldId,
    ) -> Result<Cow<'_, FieldIdentityKey>, TestAuthorityError> {
        self.fields
            .get(&field)
            .map(Cow::Borrowed)
            .ok_or(TestAuthorityError::Field(field))
    }

    fn enum_variant_key(
        &mut self,
        variant: PersistentEnumVariantId,
    ) -> Result<Cow<'_, EnumVariantIdentityKey>, TestAuthorityError> {
        self.variants
            .get(&variant)
            .map(Cow::Borrowed)
            .ok_or(TestAuthorityError::Variant(variant))
    }

    fn enum_variant_field_key(
        &mut self,
        field: PersistentEnumVariantFieldId,
    ) -> Result<Cow<'_, EnumVariantFieldKey>, TestAuthorityError> {
        self.variant_fields
            .get(&field)
            .map(Cow::Borrowed)
            .ok_or(TestAuthorityError::VariantField(field))
    }

    fn object_value_key(
        &mut self,
        value: PersistentObjectValueId,
    ) -> Result<Cow<'_, SourceDeclarationKey>, TestAuthorityError> {
        self.object_values
            .get(&value)
            .map(Cow::Borrowed)
            .ok_or(TestAuthorityError::ObjectValue(value))
    }
}

pub(super) fn nominal(
    name: &str,
    kind: SourceNominalKind,
    type_parameter_count: u32,
) -> SourceDeclarationKey {
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

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

fn source_field(owner: &SourceDeclarationKey, name: &str) -> (PersistentFieldId, FieldIdentityKey) {
    let key = FieldIdentityKey::source_declared(owner, identifier(name)).unwrap();
    let id = PersistentFieldId::from_key(&key).unwrap();
    (id, key)
}

fn source_variant(
    owner: &SourceDeclarationKey,
    name: &str,
) -> (PersistentEnumVariantId, EnumVariantIdentityKey) {
    let key = EnumVariantIdentityKey::source(owner, identifier(name)).unwrap();
    let id = PersistentEnumVariantId::from_key(&key).unwrap();
    (id, key)
}

fn variant_field(
    variant: PersistentEnumVariantId,
    selector: EnumVariantFieldSelector,
) -> (PersistentEnumVariantFieldId, EnumVariantFieldKey) {
    let key = EnumVariantFieldKey::new(variant, selector);
    let id = PersistentEnumVariantFieldId::from_key(&key).unwrap();
    (id, key)
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

pub(super) fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

pub(super) fn enum_field(
    field: PersistentEnumVariantFieldId,
    binder_index: u32,
) -> EnumSourceFieldV1 {
    EnumSourceFieldV1::new(field, binder(binder_index))
}

pub(super) fn variant(
    variant: PersistentEnumVariantId,
    style: EnumSourceVariantStyleV1,
    fields: Vec<EnumSourceFieldV1>,
) -> EnumSourceVariantV1 {
    EnumSourceVariantV1::try_new(variant, style, fields).unwrap()
}

pub(super) fn enum_shape(
    variant_id: PersistentEnumVariantId,
    style: EnumSourceVariantStyleV1,
    field_id: PersistentEnumVariantFieldId,
) -> NominalSourceShapeV1 {
    NominalSourceShapeV1::Enum(
        EnumSourceShapeV1::try_new(vec![variant(
            variant_id,
            style,
            vec![enum_field(field_id, 0)],
        )])
        .unwrap(),
    )
}
