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
            CanonicalInheritanceConstructorsV1::default(),
            CanonicalInheritanceSlotContractsV1::try_new(vec![]).unwrap(),
            CanonicalProtectedDeclarationRefsV1::try_new(vec![member]).unwrap(),
            CanonicalInheritanceSlotSchemasV1::default(),
        )
        .unwrap()
    }
}
