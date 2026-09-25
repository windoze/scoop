use super::*;

struct NestedAuthority<'a> {
    observation: Observation<'a>,
    identity: PersistentGeneratedCallableId,
    definition_path: StructuralDefinitionPath,
    function_type: SignatureTypeKey,
    captures: Vec<SignatureTypeKey>,
    sites: Vec<crate::DefaultNestedCallableSiteV1>,
}
impl ProtectedDefaultNestedCallableSemanticAuthority<&'static str> for NestedAuthority<'_> {
    fn default_nested_callable_identity_shape(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        site: crate::DefaultNestedCallableSiteV1,

        path: &WirePath,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, &'static str> {
        self.observation.check(template, path)?;
        self.sites.push(site);
        if identity != DefaultNestedCallableIdentityV1::Lambda(self.identity) {
            return Err("unknown nested source identity");
        }
        Ok(DefaultNestedCallableIdentityShapeV1::new(
            DefaultNestedCallableProvenanceV1::TemplateLexical,
            self.definition_path.clone(),
            0,
            DefaultNestedCallableBodyShapeV1::Lexical,
        ))
    }
    fn default_nested_callable_abi_shape(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        site: crate::DefaultNestedCallableSiteV1,
        arguments: DefaultNestedCallableBodyArgumentsV1<'_>,

        path: &WirePath,
    ) -> Result<DefaultNestedCallableAbiShapeV1, &'static str> {
        self.observation.check(template, path)?;
        self.sites.push(site);
        if identity != DefaultNestedCallableIdentityV1::Lambda(self.identity)
            || !matches!(arguments, DefaultNestedCallableBodyArgumentsV1::Lexical)
        {
            return Err("wrong nested body arguments");
        }
        Ok(DefaultNestedCallableAbiShapeV1::new(
            self.function_type.clone(),
            self.captures.clone(),
        ))
    }
}
#[test]
fn protected_nested_abi_preserves_identity_and_checks_complete_capture_contract() {
    let f = Fixture::new();
    let nested_path = path(StructuralDefinitionSiteRole::Lambda, 0);
    let identity = PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Lexical {
        parent: LexicalCallableParent::function(f.function),
        role: LexicalCallableRole::LambdaBody,
        path: nested_path.clone(),
    })
    .unwrap();
    let function_type = SignatureTypeKey::Function {
        effect: Effect::Ordinary,
        parameters: vec![],
        result: Box::new(value(&f)),
    };
    let lambda = DefaultLambdaV1::try_new(
        identity,
        nested_path.clone(),
        function_type.clone(),
        DefaultCallableBodyTypeArgumentsV1::lexical(),
        vec![DefaultCaptureV1::new(f.local(), value(&f), f.origin())],
        0,
    )
    .unwrap();
    let template = template(
        &f,
        vec![local(&f, f.local())],
        vec![TemplateValueParameterV1::try_new(0, f.local()).unwrap()],
        vec![],
        expression(
            &f,
            DefaultExpressionKindV1::Lambda(lambda),
            function_type.clone(),
        ),
    );
    for failure in 0..3 {
        let path = WirePath::root();
        let mut authority = NestedAuthority {
            observation: Observation::new(&template, &path),
            sites: Vec::new(),
            identity,
            definition_path: nested_path.clone(),
            function_type: if failure == 1 {
                value(&f)
            } else {
                function_type.clone()
            },
            captures: if failure == 2 {
                vec![]
            } else {
                vec![value(&f)]
            },
        };
        let result = template.validate_nested_callable_abi_semantics(&mut authority, &path);
        assert_eq!(authority.observation.calls, 2);
        assert_eq!(
            authority.sites,
            vec![crate::DefaultNestedCallableSiteV1::Body { ordinal: 0 }; 2]
        );
        match failure {
            0 => result.unwrap(),
            1 => assert!(matches!(
                result,
                Err(DefaultNestedCallableAbiValidationError::FunctionType { .. })
            )),
            2 => assert!(matches!(
                result,
                Err(DefaultNestedCallableAbiValidationError::CaptureArity { .. })
            )),
            _ => unreachable!(),
        }
    }
}
