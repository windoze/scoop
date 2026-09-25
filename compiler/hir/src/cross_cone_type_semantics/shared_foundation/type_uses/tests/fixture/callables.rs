use super::*;
use scoop_identity::{GcEffect, PropertyOwner};

#[derive(Clone, Copy)]
pub(in super::super) enum CallForm {
    Function,
    Getter,
    Setter,
}

impl Loaded {
    pub fn callable(
        &mut self,
        owner: PublicDeclarationOwnerV1,
        name: &str,
        form: CallForm,
    ) -> InheritanceCallableDeclarationV1 {
        let unit = SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id());
        let parameters = if matches!(form, CallForm::Setter) {
            vec![unit.clone()]
        } else {
            Vec::new()
        };
        self.callable_with_signature(owner, name, form, parameters, unit)
    }

    pub fn callable_with_signature(
        &mut self,
        owner: PublicDeclarationOwnerV1,
        name: &str,
        form: CallForm,
        parameters: Vec<SignatureTypeKey>,
        result: SignatureTypeKey,
    ) -> InheritanceCallableDeclarationV1 {
        let owners = match owner.nominal_owner() {
            Some(SourceNominalId::Concrete(owner)) => vec![DefinitionOwnerAtom::Type(owner)],
            _ => Vec::new(),
        };
        let site = SourceDeclarationSite::new(
            self.provider,
            PackagePath::root(),
            DefinitionOwnerChain::from_outer_to_inner(owners),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let unit = SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id());
        let receiver = (owner == PublicDeclarationOwnerV1::Extension).then_some(unit.clone());
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_external_graph_authorities(&self.identities)
            .unwrap();
        let member = match form {
            CallForm::Function => {
                let key = SourceDeclarationKey::function(
                    site,
                    CanonicalIdentifier::new(name).unwrap(),
                    0,
                    receiver.clone(),
                    parameters.clone(),
                );
                let record = CborIdentityRecord::<PersistentFunctionId, _>::from_key(key).unwrap();
                let member = InheritanceCallableDeclarationV1::Function(record.id());
                pending
                    .register_external_canonical_authority(record)
                    .unwrap();
                member
            }
            CallForm::Getter | CallForm::Setter => {
                let property = CborIdentityRecord::<PersistentPropertyId, _>::from_key(
                    SourceDeclarationKey::property(site, CanonicalIdentifier::new(name).unwrap()),
                )
                .unwrap();
                let role = if matches!(form, CallForm::Getter) {
                    AccessorRole::Getter
                } else {
                    AccessorRole::Setter
                };
                let record = CborIdentityRecord::<PersistentPropertyAccessorId, _>::from_key(
                    PropertyAccessorKey::new(PropertyOwner::Property(property.id()), role),
                )
                .unwrap();
                let member = match role {
                    AccessorRole::Getter => InheritanceCallableDeclarationV1::Getter(record.id()),
                    AccessorRole::Setter => InheritanceCallableDeclarationV1::Setter(record.id()),
                };
                if self
                    .identities
                    .canonical_key::<_, SourceDeclarationKey>(property.id())
                    .is_err()
                {
                    pending
                        .register_external_canonical_authority(property)
                        .unwrap();
                }
                pending
                    .register_external_canonical_authority(record)
                    .unwrap();
                member
            }
        };
        self.identities = pending.finish().unwrap();
        self.publish_callable(origin(member), owner, receiver, parameters, result);
        member
    }

    pub(super) fn publish_callable(
        &mut self,
        target: CallableTemplateOrigin,
        owner: PublicDeclarationOwnerV1,
        receiver: Option<SignatureTypeKey>,
        parameters: Vec<SignatureTypeKey>,
        result: SignatureTypeKey,
    ) {
        let declaration = CallableDeclarationRecordV1::try_new(
            target,
            owner,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            receiver,
            CanonicalSourceParameterShapesV1::try_new(
                parameters
                    .into_iter()
                    .enumerate()
                    .map(|(index, ty)| {
                        SourceParameterShapeV1::new(
                            CanonicalIdentifier::new(&format!("p{index}")).unwrap(),
                            ty,
                        )
                    })
                    .collect(),
            )
            .unwrap(),
            result,
            CallableSourceEffectsV1::try_new(
                Effect::Ordinary,
                CallableSafetyV1::Safe,
                GcEffect::Managed,
                CallableImplementationV1::Scoop,
                CallableOperatorRoleV1::None,
                CallableInfixV1::Ordinary,
            )
            .unwrap(),
            CallableModalityV1::Final,
            DeclaredVisibilityV1::Public,
            CanonicalPersistentIdsV1::empty(),
        )
        .unwrap();
        let mut declarations = self
            .public
            .callable_interfaces()
            .all_declarations()
            .cloned()
            .collect::<Vec<_>>();
        declarations.push(declaration);
        self.public = with_tables(
            &self.public,
            CanonicalCallableInterfacesV1::with_support(Vec::new(), declarations).unwrap(),
            self.public.external_references().clone(),
        );
    }
}

pub(in super::super) fn origin(member: InheritanceCallableDeclarationV1) -> CallableTemplateOrigin {
    match member {
        InheritanceCallableDeclarationV1::Function(id) => CallableTemplateOrigin::Function(id),
        InheritanceCallableDeclarationV1::Getter(id)
        | InheritanceCallableDeclarationV1::Setter(id) => CallableTemplateOrigin::Accessor(id),
    }
}

pub(super) fn with_tables(
    public: &CrossConeHirInterfaceSectionV1,
    callables: CanonicalCallableInterfacesV1,
    references: CanonicalExternalHirReferencesV1,
) -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        public.public_bindings().clone(),
        public.nominal_interfaces().clone(),
        callables,
        public.property_interfaces().clone(),
        public.type_aliases().clone(),
        public.source_interfaces().clone(),
        public.default_templates().clone(),
        public.constants().clone(),
        public.definition_sources().clone(),
        references,
    )
}
