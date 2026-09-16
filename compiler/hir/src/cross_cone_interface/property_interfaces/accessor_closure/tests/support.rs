use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    CoreBuiltinNominal, DeclarationScope, DefinitionOwnerAtom, DefinitionOwnerChain, Effect,
    GcEffect, NominalDeclarationOwner, PackagePath, PersistentExtensionPropertyId,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId, PropertyAccessorKey,
    PropertyOwner, SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind,
};

use crate::{
    CallableImplementationV1, CallableInfixV1, CallableInterfaceRecordV1, CallableModalityV1,
    CallableOperatorRoleV1, CallableSafetyV1, CallableSourceEffectsV1, CanonicalBinderListV1,
    CanonicalCallableInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalSourceParameterShapesV1,
    PropertyAccessorClosureValidationError, PropertyCapabilityV1, PropertyDeclarationId,
    PropertyInterfaceRecordV1, PropertyPublicAccessV1, PropertyRepresentationV1,
    PropertySetterPublicAccessV1, PublicDeclarationOwnerV1, PublicLookupAccessV1,
    SourceParameterShapeV1,
};

pub(super) fn validate(
    properties: Vec<PropertyInterfaceRecordV1>,
    callables: Vec<CallableInterfaceRecordV1>,
) -> Result<(), PropertyAccessorClosureValidationError> {
    CanonicalPropertyInterfacesV1::try_new(properties)
        .unwrap()
        .validate_accessor_closure(&CanonicalCallableInterfacesV1::try_new(callables).unwrap())
}

pub(super) struct Fixture {
    pub(super) declaration: PropertyDeclarationId,
    pub(super) owner: PublicDeclarationOwnerV1,
    pub(super) receiver: Option<SignatureTypeKey>,
    pub(super) value_type: SignatureTypeKey,
    pub(super) getter: PersistentPropertyAccessorId,
    pub(super) setter: PersistentPropertyAccessorId,
}

impl Fixture {
    pub(super) fn top_level(name: &str) -> Self {
        let property: CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(SourceDeclarationKey::property(
                top_level_site(),
                identifier(name),
            ))
            .unwrap();
        Self::new(
            PropertyOwner::Property(property.id()),
            PublicDeclarationOwnerV1::TopLevel,
            None,
        )
    }

    pub(super) fn nominal(name: &str, kind: SourceNominalKind) -> Self {
        let nominal: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
                top_level_site(),
                identifier(&format!("{name}Owner")),
                kind,
                0,
            ))
            .unwrap();
        let property: CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(SourceDeclarationKey::property(
                owned_site(DefinitionOwnerAtom::Type(nominal.id())),
                identifier(name),
            ))
            .unwrap();
        Self::new(
            PropertyOwner::Property(property.id()),
            PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::Concrete(nominal.id())),
            None,
        )
    }

    pub(super) fn extension(name: &str) -> Self {
        let receiver = any_type();
        let property: CborIdentityRecord<PersistentExtensionPropertyId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(SourceDeclarationKey::extension_property(
                top_level_site(),
                identifier(name),
                0,
                receiver.clone(),
            ))
            .unwrap();
        Self::new(
            PropertyOwner::ExtensionProperty(property.id()),
            PublicDeclarationOwnerV1::Extension,
            Some(receiver),
        )
    }

    fn new(
        declaration: PropertyDeclarationId,
        owner: PublicDeclarationOwnerV1,
        receiver: Option<SignatureTypeKey>,
    ) -> Self {
        let getter = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
            declaration,
            AccessorRole::Getter,
        ))
        .unwrap();
        let setter = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
            declaration,
            AccessorRole::Setter,
        ))
        .unwrap();
        Self {
            declaration,
            owner,
            receiver,
            value_type: any_type(),
            getter,
            setter,
        }
    }

    pub(super) fn property(
        &self,
        setter_access: Option<PropertySetterPublicAccessV1>,
        representation: PropertyRepresentationV1,
        access: PropertyPublicAccessV1,
    ) -> PropertyInterfaceRecordV1 {
        let capability = match setter_access {
            None => PropertyCapabilityV1::read_only(self.getter),
            Some(setter_access) => {
                PropertyCapabilityV1::try_read_write(self.getter, self.setter, setter_access)
                    .unwrap()
            }
        };
        PropertyInterfaceRecordV1::try_new(
            self.declaration,
            self.owner,
            empty_binders(),
            self.receiver.clone(),
            self.value_type.clone(),
            capability,
            representation,
            access,
        )
        .unwrap()
    }

    pub(super) fn accessor(&self, role: AccessorRole) -> AccessorInput {
        let (accessor, parameters, result) = match role {
            AccessorRole::Getter => (self.getter, Vec::new(), self.value_type.clone()),
            AccessorRole::Setter => (self.setter, vec![self.value_type.clone()], unit_type()),
        };
        AccessorInput {
            accessor,
            owner: self.owner,
            receiver: self.receiver.clone(),
            parameters,
            result,
            effects: scoop_effects(),
            modality: CallableModalityV1::Final,
            access: PublicLookupAccessV1::DirectOnly,
        }
    }
}

pub(super) struct AccessorInput {
    accessor: PersistentPropertyAccessorId,
    pub(super) owner: PublicDeclarationOwnerV1,
    pub(super) receiver: Option<SignatureTypeKey>,
    pub(super) parameters: Vec<SignatureTypeKey>,
    pub(super) result: SignatureTypeKey,
    pub(super) effects: CallableSourceEffectsV1,
    pub(super) modality: CallableModalityV1,
    pub(super) access: PublicLookupAccessV1,
}

impl AccessorInput {
    pub(super) fn build(self) -> CallableInterfaceRecordV1 {
        let parameters = self
            .parameters
            .into_iter()
            .enumerate()
            .map(|(index, value_type)| {
                SourceParameterShapeV1::new(identifier(&format!("argument{index}")), value_type)
            })
            .collect();
        CallableInterfaceRecordV1::try_new(
            CallableTemplateOrigin::Accessor(self.accessor),
            self.owner,
            empty_binders(),
            self.receiver,
            CanonicalSourceParameterShapesV1::try_new(parameters).unwrap(),
            self.result,
            self.effects,
            self.modality,
            self.access,
        )
        .unwrap()
    }
}

pub(super) fn effects(
    execution: Effect,
    implementation: CallableImplementationV1,
    operator_role: CallableOperatorRoleV1,
    infix: CallableInfixV1,
) -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        execution,
        CallableSafetyV1::Safe,
        GcEffect::Managed,
        implementation,
        operator_role,
        infix,
    )
    .unwrap()
}

fn scoop_effects() -> CallableSourceEffectsV1 {
    effects(
        Effect::Ordinary,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
}

pub(super) fn empty_binders() -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(Vec::new()).unwrap()
}

fn any_type() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(CoreBuiltinNominal::Any.identity_record().id())
}

pub(super) fn unit_type() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
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
