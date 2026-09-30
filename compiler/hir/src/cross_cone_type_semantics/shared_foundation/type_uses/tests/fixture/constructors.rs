use super::*;

impl Loaded {
    pub fn constructor(
        &mut self,
        owner: PersistentTypeId,
        parameters: Vec<SignatureTypeKey>,
    ) -> CallableTemplateOrigin {
        let site = SourceDeclarationSite::new(
            self.provider,
            PackagePath::root(),
            DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(owner)]),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let identity = CborIdentityRecord::<PersistentConstructorId, _>::from_key(
            SourceDeclarationKey::constructor(site, parameters.clone()),
        )
        .unwrap();
        let target = CallableTemplateOrigin::Constructor(identity.id());
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_external_graph_authorities(&self.identities)
            .unwrap();
        pending
            .register_external_canonical_authority(identity.clone())
            .unwrap();
        self.identities = pending.finish().unwrap();
        let source = self
            .public
            .nominal_interfaces()
            .declaration(SourceNominalId::Concrete(owner))
            .unwrap();
        let mut constructors = source.constructors().values().to_vec();
        constructors.push(identity.id());
        self.construction_shape(owner, constructors, source.source_shape().clone());
        self.publish_callable(
            target,
            PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner)),
            None,
            parameters,
            SignatureTypeKey::Nominal(owner),
        );
        target
    }

    pub fn variant(
        &mut self,
        owner: PersistentTypeId,
        name: &str,
        parameters: Vec<SignatureTypeKey>,
    ) -> CallableTemplateOrigin {
        let key = self
            .identities
            .canonical_key::<_, SourceDeclarationKey>(owner)
            .unwrap();
        let identity = CborIdentityRecord::<PersistentEnumVariantId, _>::from_key(
            EnumVariantIdentityKey::source(&key, CanonicalIdentifier::new(name).unwrap()).unwrap(),
        )
        .unwrap();
        let target = CallableTemplateOrigin::VariantConstructor(identity.id());
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_external_graph_authorities(&self.identities)
            .unwrap();
        pending
            .register_external_canonical_authority(identity.clone())
            .unwrap();
        let fields = parameters
            .iter()
            .enumerate()
            .map(|(index, ty)| {
                let field = CborIdentityRecord::<PersistentEnumVariantFieldId, _>::from_key(
                    EnumVariantFieldKey::new(
                        identity.id(),
                        EnumVariantFieldSelector::Positional {
                            declaration_index: index as u32,
                        },
                    ),
                )
                .unwrap();
                pending
                    .register_external_canonical_authority(field.clone())
                    .unwrap();
                EnumSourceFieldV1::new(field.id(), ty.clone())
            })
            .collect::<Vec<_>>();
        self.identities = pending.finish().unwrap();
        let source = self
            .public
            .nominal_interfaces()
            .declaration(SourceNominalId::Concrete(owner))
            .unwrap();
        let NominalSourceShapeV1::Enum(shape) = source.source_shape() else {
            panic!("variant fixtures require an enum owner");
        };
        let mut variants = shape.variants().to_vec();
        let style = if fields.is_empty() {
            EnumSourceVariantStyleV1::Unit
        } else {
            EnumSourceVariantStyleV1::Positional
        };
        variants.push(EnumSourceVariantV1::try_new(identity.id(), style, fields).unwrap());
        self.construction_shape(
            owner,
            Vec::new(),
            NominalSourceShapeV1::Enum(EnumSourceShapeV1::try_new(variants).unwrap()),
        );
        self.publish_callable(
            target,
            PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner)),
            None,
            parameters,
            SignatureTypeKey::Nominal(owner),
        );
        target
    }

    fn construction_shape(
        &mut self,
        owner: PersistentTypeId,
        constructors: Vec<PersistentConstructorId>,
        shape: NominalSourceShapeV1,
    ) {
        let source = self
            .public
            .nominal_interfaces()
            .declaration(SourceNominalId::Concrete(owner))
            .unwrap();
        let details = source.declaration_details();
        let primary = if source.kind() == PublicNominalKindV1::Struct {
            details
                .primary_value_constructor()
                .or_else(|| constructors.first().copied())
        } else {
            None
        };
        let constructors = CanonicalPersistentIdsV1::try_new(constructors).unwrap();
        let declaration = NominalInterfaceRecordV1::try_new(
            source.declaration(),
            source.kind(),
            source.type_parameters().clone(),
            source.exact_supertypes().clone(),
            constructors.clone(),
            source.members().clone(),
            source.nested_bindings().clone(),
            shape,
            NominalDeclarationDetailsV1::new(
                details.modality(),
                details.declared_visibility(),
                constructors,
                details.members().clone(),
                details.children().clone(),
                details.dispatch_order().clone(),
                details.dispatch_selections().clone(),
                primary,
                details.instantiation_conditions().clone(),
            ),
        )
        .unwrap();
        let mut nominals = self
            .public
            .nominal_interfaces()
            .all_records()
            .cloned()
            .collect::<Vec<_>>();
        let target = declaration.declaration();
        *nominals
            .iter_mut()
            .find(|record| record.declaration() == target)
            .unwrap() = declaration;
        self.public = CrossConeHirInterfaceSectionV1::new(
            self.public.public_bindings().clone(),
            CanonicalNominalInterfacesV1::try_new(nominals).unwrap(),
            self.public.callable_interfaces().clone(),
            self.public.property_interfaces().clone(),
            self.public.type_aliases().clone(),
            self.public.source_interfaces().clone(),
            self.public.default_templates().clone(),
            self.public.constants().clone(),
            self.public.definition_sources().clone(),
            self.public.external_references().clone(),
            self.public.generic_callable_bodies().clone(),
            self.public.generic_initializations().clone(),
            self.public.generic_delegates().clone(),
        );
    }
}
