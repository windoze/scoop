use super::*;

impl Fixture {
    pub(super) fn declaration(&self) -> ProtectedDeclarationRefV1 {
        ProtectedDeclarationRefV1::Callable(
            ProtectedCallableDeclarationRefV1::try_new(CallableTemplateOrigin::Function(
                self.function.id(),
            ))
            .unwrap(),
        )
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
            CanonicalInheritanceSlotContractsV1::try_new(vec![]).unwrap(),
            CanonicalProtectedDeclarationRefsV1::try_new(vec![member]).unwrap(),
            CanonicalInheritanceSlotSchemasV1::default(),
        )
        .unwrap()
    }
}
