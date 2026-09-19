use super::*;

impl Fixture {
    pub(super) fn declaration(&self) -> ProtectedDeclarationInterfaceV1 {
        let origin = CallableTemplateOrigin::Function(self.function.id());
        let owner = SourceNominalId::Concrete(self.owner.id());
        ProtectedDeclarationInterfaceV1::Callable(Box::new(
            ProtectedCallableInterfaceV1::try_new(
                origin,
                DeclarationAccessSourceV1::try_new(
                    DeclaredVisibilityV1::Protected,
                    vec![owner],
                    self.origin.clone(),
                )
                .unwrap(),
                ProtectedCallablePayloadV1::try_new(
                    origin,
                    owner,
                    CanonicalBinderListV1::try_new(vec![]).unwrap(),
                    CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
                        name("value"),
                        self.value_type(),
                    )])
                    .unwrap(),
                    self.value_type(),
                    CallableSourceEffectsV1::try_new(
                        Effect::Ordinary,
                        CallableSafetyV1::Safe,
                        scoop_identity::GcEffect::Managed,
                        CallableImplementationV1::Scoop,
                        CallableOperatorRoleV1::None,
                        CallableInfixV1::Ordinary,
                    )
                    .unwrap(),
                    CallableModalityV1::Final,
                    CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        ))
    }
    pub(super) fn inheritance(
        &self,
        member: ProtectedDeclarationRefV1,
    ) -> NominalInheritanceInterfaceV1 {
        NominalInheritanceInterfaceV1::try_new(
            NominalInheritanceEdgesV1::try_new(
                self.exact.id(),
                NominalInheritanceModalityV1::Final,
                DirectClassBaseV1::NoClassBase,
                vec![],
            )
            .unwrap(),
            NominalAccessDomainsV1::new(
                PersistentLookupDomainV1::new(PersistentAccessDomainV1::universal()),
                PersistentInheritanceDomainV1::new(PersistentAccessDomainV1::empty()),
                PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::empty()),
            ),
            CanonicalInheritanceConstructorsV1::default(),
            CanonicalInheritanceSlotContractsV1::try_new(vec![]).unwrap(),
            CanonicalProtectedDeclarationRefsV1::try_new(vec![member]).unwrap(),
            CanonicalInheritanceSlotSchemasV1::default(),
        )
        .unwrap()
    }
    pub(super) fn protocol(&self) -> ProtectedCallableSourceInterfaceV1 {
        ProtectedCallableSourceInterfaceV1::try_new(
            self.key().owner(),
            CanonicalProtectedSourceParametersV1::try_new(vec![ProtectedSourceParameterV1::new(
                name("value"),
                self.value_type(),
                ProtectedParameterCallingV1::Default {
                    template: self.key(),
                },
                self.origin.clone(),
            )])
            .unwrap(),
        )
        .unwrap()
    }
    pub(super) fn template(&self) -> ProtectedDefaultTemplateV1 {
        let receiver_type = SignatureTypeKey::Nominal(self.owner.id());
        ProtectedDefaultTemplateV1::try_new(
            self.key(),
            PersistentLexicalRootV1::Function(self.function.id()),
            StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
                [],
            ),
            CanonicalTemplateLocalTableV1::try_new(vec![
                TemplateLocalRecordV1::try_new(
                    LocalValueSelector::This,
                    receiver_type.clone(),
                    CanonicalBooleanV1::False,
                    TemplateLocalDefinitionV1::Source(self.origin.clone()),
                )
                .unwrap(),
            ])
            .unwrap(),
            ExportDefaultBodyV1::try_new(
                vec![],
                DefaultExpressionV1::try_new(
                    DefaultExpressionKindV1::UnitLiteral,
                    self.value_type(),
                    self.origin.clone(),
                )
                .unwrap(),
            )
            .unwrap(),
            self.value_type(),
            CanonicalBooleanV1::False,
            CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
            OptionalTemplateReceiverV1::Present(
                TemplateReceiverV1::try_new(LocalValueSelector::This, receiver_type).unwrap(),
            ),
            CanonicalTemplateValueParametersV1::try_new(vec![]).unwrap(),
            ProtectedDefaultReferenceSetV1::try_new(vec![], vec![], vec![], vec![], vec![], vec![])
                .unwrap(),
            self.origin.clone(),
        )
        .unwrap()
    }
}
