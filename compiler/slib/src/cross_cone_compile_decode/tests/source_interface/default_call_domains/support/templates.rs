use super::*;
use scoop_identity::{
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

impl Builder {
    pub fn template(
        &self,
        publisher: CallableTemplateOrigin,
        provider: CallableTemplateOrigin,
        arguments: Vec<SignatureTypeKey>,
        domains: Option<(Domain, Option<Domain>, Domain)>,
    ) -> ExportDefaultTemplateV1 {
        let origin = self.body_origin(provider);
        let references = references(publisher, &self.token, &origin, domains);
        ExportDefaultTemplateV1::try_new(
            ExportDefaultTemplateKeyV1::new(publisher, 0),
            PersistentLexicalRootV1::try_from(provider).unwrap(),
            StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
                [],
            ),
            CanonicalTemplateLocalTableV1::try_new(vec![]).unwrap(),
            ExportDefaultBodyV1::try_new(
                vec![],
                DefaultExpressionV1::try_new(
                    DefaultExpressionKindV1::StructConstruct {
                        owner_type: self.token.clone(),
                        fields: vec![],
                    },
                    self.token.clone(),
                    origin.clone(),
                )
                .unwrap(),
            )
            .unwrap(),
            self.token.clone(),
            CanonicalBooleanV1::False,
            CanonicalBinderUseListV1::try_new(arguments).unwrap(),
            OptionalTemplateReceiverV1::Absent,
            CanonicalTemplateValueParametersV1::try_new(vec![]).unwrap(),
            references,
            origin,
        )
        .unwrap()
    }
}

impl Fixture {
    pub fn change_witness(&mut self, direct: Domain, slot: Option<Domain>, target: Domain) {
        let template = &self.interface.default_templates().records()[0];
        let references = references(
            template.key().owner(),
            template.result(),
            template.definition_origin(),
            Some((direct, slot, target)),
        );
        let replacement = copy(template, template.type_parameters().clone(), references);
        self.replace(
            self.interface.callable_interfaces().clone(),
            CanonicalExportDefaultTemplatesV1::try_new(vec![replacement]).unwrap(),
        );
    }

    pub fn change_mapping(&mut self, arguments: Vec<SignatureTypeKey>) {
        let template = &self.interface.default_templates().records()[0];
        let replacement = copy(
            template,
            CanonicalBinderUseListV1::try_new(arguments).unwrap(),
            template.references().clone(),
        );
        self.replace(
            self.interface.callable_interfaces().clone(),
            CanonicalExportDefaultTemplatesV1::try_new(vec![replacement]).unwrap(),
        );
    }
}

fn references(
    owner: CallableTemplateOrigin,
    ty: &SignatureTypeKey,
    origin: &ExportDefinitionSourceV1,
    domains: Option<(Domain, Option<Domain>, Domain)>,
) -> ExportDefaultReferenceSetV1 {
    let types = domains
        .map(|(direct, slot, target)| {
            ExportDefaultReferenceV1::new(
                ty.clone(),
                origin.clone(),
                ExportDefaultAccessWitnessV1::try_new(owner, direct, slot, target).unwrap(),
            )
        })
        .into_iter()
        .collect();
    ExportDefaultReferenceSetV1::try_new(vec![], vec![], types, vec![], vec![], vec![]).unwrap()
}

fn copy(
    t: &ExportDefaultTemplateV1,
    mapping: CanonicalBinderUseListV1,
    references: ExportDefaultReferenceSetV1,
) -> ExportDefaultTemplateV1 {
    ExportDefaultTemplateV1::try_new(
        t.key(),
        t.definition_root(),
        t.definition_path().clone(),
        t.locals().clone(),
        t.body().clone(),
        t.result().clone(),
        t.allows_suspend(),
        mapping,
        t.receiver().clone(),
        t.value_parameters().clone(),
        references,
        t.definition_origin().clone(),
    )
    .unwrap()
}
