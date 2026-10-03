use crate::{
    PropertyAccessorImplementationV1 as AccessorForm, PropertyAccessorSourceV1 as AccessorSource,
    PropertyAccessorsV1 as Accessors,
};

use std::collections::BTreeMap;

use scoop_identity::{
    AccessorRole, CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerAtom,
    DefinitionOwnerChain, NominalDeclarationOwner, NonEmptyVec, PackagePath,
    PersistentExtensionPropertyId, PersistentGenericTypeId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentTypeId, PropertyAccessorKey, PropertyOwner, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

use super::super::*;
use crate::{
    CanonicalBinderListV1, CanonicalSignatureTypesV1, NominalInterfaceShapeAuthority,
    NominalTypeParameterBoundsV1, PropertyCapabilityV1, PropertyPublicAccessV1,
    PropertyRepresentationV1, PropertySetterPublicAccessV1, PublicNominalKindV1,
    PublicNominalShapeV1, TypeParameterBinderV1, TypeParameterBoundsV1,
};

pub(super) struct Fixture {
    pub(super) declaration: PropertyOwner,
    pub(super) getter: PersistentPropertyAccessorId,
    pub(super) setter: PersistentPropertyAccessorId,
    pub(super) other_property: PersistentPropertyId,
    pub(super) contract: PersistentGenericTypeId,
    pub(super) result: PersistentTypeId,
    identity: PropertyDeclarationIdentityShapeV1,
    source: PropertyDeclarationSourceShapeV1,
    accessors: BTreeMap<PersistentPropertyAccessorId, PropertyAccessorKey>,
    generic_nominals: BTreeMap<PersistentGenericTypeId, PublicNominalShapeV1>,
    concrete_nominals: BTreeMap<PersistentTypeId, PublicNominalShapeV1>,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let extension_key = SourceDeclarationKey::extension_property(
            top_level_site(),
            identifier("content"),
            1,
            own_binder(0),
        );
        let extension =
            PersistentExtensionPropertyId::from_source_declaration(&extension_key).unwrap();
        let declaration = PropertyOwner::ExtensionProperty(extension);
        let getter = accessor(declaration, AccessorRole::Getter);
        let setter = accessor(declaration, AccessorRole::Setter);
        let other_key = SourceDeclarationKey::property(top_level_site(), identifier("other"));
        let other_property = PersistentPropertyId::from_source_declaration(&other_key).unwrap();
        let contract_key = nominal("Contract", SourceNominalKind::Interface, 1);
        let contract = PersistentGenericTypeId::from_source_declaration(&contract_key).unwrap();
        let result_key = nominal("Result", SourceNominalKind::Struct, 0);
        let result = PersistentTypeId::from_source_declaration(&result_key).unwrap();
        let capability = PropertyCapabilityV1::try_read_write(
            getter,
            setter,
            PropertySetterPublicAccessV1::Public,
        )
        .unwrap();
        Self {
            declaration,
            getter,
            setter,
            other_property,
            contract,
            result,
            identity: PropertyDeclarationIdentityShapeV1::new(
                PublicDeclarationOwnerV1::Extension,
                1,
                0,
                Some(own_binder(0)),
            ),
            source: PropertyDeclarationSourceShapeV1::new(
                capability,
                PropertyRepresentationV1::RuntimeAccessor,
                PropertyPublicAccessV1::DirectOnly,
            ),
            accessors: BTreeMap::from([
                (
                    getter,
                    PropertyAccessorKey::new(declaration, AccessorRole::Getter),
                ),
                (
                    setter,
                    PropertyAccessorKey::new(declaration, AccessorRole::Setter),
                ),
            ]),
            generic_nominals: BTreeMap::from([(
                contract,
                PublicNominalShapeV1::new(PublicNominalKindV1::Interface, 1),
            )]),
            concrete_nominals: BTreeMap::from([(
                result,
                PublicNominalShapeV1::new(PublicNominalKindV1::Struct, 0),
            )]),
        }
    }

    pub(super) fn capability(&self) -> PropertyCapabilityV1 {
        self.source.capability()
    }

    pub(super) fn record(&self) -> PropertyInterfaceRecordV1 {
        self.record_with(
            binders_with_bound(self.contract, own_binder(0)),
            Some(own_binder(0)),
            SignatureTypeKey::Nominal(self.result),
        )
    }

    pub(super) fn record_with(
        &self,
        type_parameters: CanonicalBinderListV1,
        receiver: Option<SignatureTypeKey>,
        value_type: SignatureTypeKey,
    ) -> PropertyInterfaceRecordV1 {
        PropertyInterfaceRecordV1::try_new(
            self.declaration,
            PublicDeclarationOwnerV1::Extension,
            type_parameters,
            receiver,
            value_type,
            Accessors::try_read_write(
                AccessorSource::new(self.getter, AccessorForm::Body),
                AccessorSource::new(self.setter, AccessorForm::Body),
            )
            .unwrap(),
            PropertyRepresentationV1::RuntimeAccessor,
            PropertyPublicAccessV1::DirectOnly,
            crate::PropertySetterPublicAccessV1::Public,
        )
        .unwrap()
    }

    pub(super) fn authority(&self) -> TestAuthority {
        TestAuthority {
            declaration: self.declaration,
            identity: self.identity.clone(),
            source: self.source,
            accessors: self.accessors.clone(),
            generic_nominals: self.generic_nominals.clone(),
            concrete_nominals: self.concrete_nominals.clone(),
        }
    }

    pub(super) fn missing_result(&self) -> PersistentTypeId {
        PersistentTypeId::from_source_declaration(&nominal("Missing", SourceNominalKind::Struct, 0))
            .unwrap()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestAuthorityError {
    Property(PropertyOwner),
    Source(PropertyOwner),
    Accessor(PersistentPropertyAccessorId),
    Concrete(PersistentTypeId),
    Generic(PersistentGenericTypeId),
}

impl std::fmt::Display for TestAuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing test authority: {self:?}")
    }
}

impl std::error::Error for TestAuthorityError {}

pub(super) struct TestAuthority {
    pub(super) declaration: PropertyOwner,
    pub(super) identity: PropertyDeclarationIdentityShapeV1,
    pub(super) source: PropertyDeclarationSourceShapeV1,
    pub(super) accessors: BTreeMap<PersistentPropertyAccessorId, PropertyAccessorKey>,
    pub(super) generic_nominals: BTreeMap<PersistentGenericTypeId, PublicNominalShapeV1>,
    pub(super) concrete_nominals: BTreeMap<PersistentTypeId, PublicNominalShapeV1>,
}

impl NominalInterfaceShapeAuthority<TestAuthorityError> for TestAuthority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        self.concrete_nominals
            .get(&declaration)
            .copied()
            .ok_or(TestAuthorityError::Concrete(declaration))
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        self.generic_nominals
            .get(&declaration)
            .copied()
            .ok_or(TestAuthorityError::Generic(declaration))
    }
}

impl PropertyInterfaceSemanticAuthority<TestAuthorityError> for TestAuthority {
    fn property_declaration_identity_shape(
        &mut self,
        declaration: PropertyOwner,
    ) -> Result<PropertyDeclarationIdentityShapeV1, TestAuthorityError> {
        if declaration == self.declaration {
            Ok(self.identity.clone())
        } else {
            Err(TestAuthorityError::Property(declaration))
        }
    }

    fn property_declaration_source_shape(
        &mut self,
        declaration: PropertyOwner,
    ) -> Result<PropertyDeclarationSourceShapeV1, TestAuthorityError> {
        if declaration == self.declaration {
            Ok(self.source)
        } else {
            Err(TestAuthorityError::Source(declaration))
        }
    }

    fn property_accessor_key(
        &mut self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<PropertyAccessorKey, TestAuthorityError> {
        self.accessors
            .get(&accessor)
            .copied()
            .ok_or(TestAuthorityError::Accessor(accessor))
    }
}

pub(super) fn generic_member_fixture() -> (PropertyInterfaceRecordV1, TestAuthority) {
    let owner_key = nominal("Box", SourceNominalKind::Class, 1);
    let owner = PersistentGenericTypeId::from_source_declaration(&owner_key).unwrap();
    let declaration_key = SourceDeclarationKey::property(
        owned_site(DefinitionOwnerAtom::GenericType(owner)),
        identifier("value"),
    );
    let property = PersistentPropertyId::from_source_declaration(&declaration_key).unwrap();
    let declaration = PropertyOwner::Property(property);
    let getter = accessor(declaration, AccessorRole::Getter);
    let owner_ref =
        PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::GenericTemplate(owner));
    let capability = PropertyCapabilityV1::read_only(getter);
    let record = PropertyInterfaceRecordV1::try_new(
        declaration,
        owner_ref,
        empty_binders(),
        None,
        own_binder(0),
        Accessors::read_only(AccessorSource::new(getter, AccessorForm::Body)),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::PublicSlot,
        crate::PropertySetterPublicAccessV1::Restricted,
    )
    .unwrap();
    let authority = TestAuthority {
        declaration,
        identity: PropertyDeclarationIdentityShapeV1::new(owner_ref, 0, 1, None),
        source: PropertyDeclarationSourceShapeV1::new(
            capability,
            PropertyRepresentationV1::RuntimeAccessor,
            PropertyPublicAccessV1::PublicSlot,
        ),
        accessors: BTreeMap::from([(
            getter,
            PropertyAccessorKey::new(declaration, AccessorRole::Getter),
        )]),
        generic_nominals: BTreeMap::from([(
            owner,
            PublicNominalShapeV1::new(PublicNominalKindV1::Class, 1),
        )]),
        concrete_nominals: BTreeMap::new(),
    };
    (record, authority)
}

pub(super) fn const_object_fixture() -> (PropertyInterfaceRecordV1, TestAuthority, PersistentTypeId)
{
    let owner_key = nominal("Constants", SourceNominalKind::Object, 0);
    let owner = PersistentTypeId::from_source_declaration(&owner_key).unwrap();
    let declaration_key = SourceDeclarationKey::property(
        owned_site(DefinitionOwnerAtom::Type(owner)),
        identifier("answer"),
    );
    let property = PersistentPropertyId::from_source_declaration(&declaration_key).unwrap();
    let declaration = PropertyOwner::Property(property);
    let getter = accessor(declaration, AccessorRole::Getter);
    let value_key = nominal("ConstValue", SourceNominalKind::Struct, 0);
    let value = PersistentTypeId::from_source_declaration(&value_key).unwrap();
    let owner_ref = PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::Concrete(owner));
    let capability = PropertyCapabilityV1::read_only(getter);
    let record = PropertyInterfaceRecordV1::try_new(
        declaration,
        owner_ref,
        empty_binders(),
        None,
        SignatureTypeKey::Nominal(value),
        Accessors::read_only(AccessorSource::new(getter, AccessorForm::Constant)),
        PropertyRepresentationV1::Const,
        PropertyPublicAccessV1::DirectOnly,
        crate::PropertySetterPublicAccessV1::Restricted,
    )
    .unwrap();
    let authority = TestAuthority {
        declaration,
        identity: PropertyDeclarationIdentityShapeV1::new(owner_ref, 0, 0, None),
        source: PropertyDeclarationSourceShapeV1::new(
            capability,
            PropertyRepresentationV1::Const,
            PropertyPublicAccessV1::DirectOnly,
        ),
        accessors: BTreeMap::from([(
            getter,
            PropertyAccessorKey::new(declaration, AccessorRole::Getter),
        )]),
        generic_nominals: BTreeMap::new(),
        concrete_nominals: BTreeMap::from([
            (
                owner,
                PublicNominalShapeV1::new(PublicNominalKindV1::Object, 0),
            ),
            (
                value,
                PublicNominalShapeV1::new(PublicNominalKindV1::Struct, 0),
            ),
        ]),
    };
    (record, authority, owner)
}

pub(super) fn binders_with_bound(
    contract: PersistentGenericTypeId,
    argument: SignatureTypeKey,
) -> CanonicalBinderListV1 {
    let bounds = NominalTypeParameterBoundsV1::try_new(
        None,
        CanonicalSignatureTypesV1::try_new(vec![SignatureTypeKey::NominalApplication {
            origin: contract,
            arguments: NonEmptyVec::new(vec![argument]).unwrap(),
        }])
        .unwrap(),
    )
    .unwrap();
    CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
        identifier("T"),
        TypeParameterBoundsV1::Nominal(bounds),
    )])
    .unwrap()
}

fn empty_binders() -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(Vec::new()).unwrap()
}

fn accessor(owner: PropertyOwner, role: AccessorRole) -> PersistentPropertyAccessorId {
    PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(owner, role)).unwrap()
}

pub(super) const fn own_binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

pub(super) const fn deep_binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 1, index }
}

fn nominal(name: &str, kind: SourceNominalKind, arity: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(top_level_site(), identifier(name), kind, arity)
}

fn top_level_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
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
