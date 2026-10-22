use super::*;
use std::sync::Arc;

#[test]
fn applied_defaults_preserve_local_generic_scope_and_independent_requirement_fragments() {
    let root =
        PersistentGenericFunctionId::from_source_declaration(&function_key("defaultOwner", 1))
            .unwrap();
    let local =
        PersistentGenericFunctionId::from_source_declaration(&function_key("local", 1)).unwrap();
    let parameter = LocalValueSelector::Parameter {
        declaration_index: 0,
    };
    let locals = hir::CanonicalTemplateLocalTableV1::try_new(vec![
        hir::TemplateLocalRecordV1::try_new(
            parameter.clone(),
            binder(0, 0),
            hir::CanonicalBooleanV1::False,
            hir::TemplateLocalDefinitionV1::Source(origin()),
        )
        .unwrap(),
    ])
    .unwrap();
    let value = expression(
        hir::DefaultExpressionKindV1::Local(parameter.clone()),
        binder(0, 0),
    );
    let local_function = hir::DefaultLocalFunctionV1::try_new(
        CallableTemplateOrigin::GenericFunction(local),
        path(StructuralDefinitionSiteRole::LocalDeclaration, 0),
        SignatureTypeKey::Function {
            effect: Effect::Ordinary,
            receiver: None,
            parameters: vec![binder(1, 0), binder(0, 0)],
            result: Box::new(binder(1, 0)),
        },
        vec![],
        1,
    )
    .unwrap();
    let fragment = hir::ExportTemplateFragmentV1::new(locals.clone(), vec![], vec![value.clone()]);
    let binding = hir::DefaultRequirementBindingValueV1::Implementation {
        body: fragment,
        inputs: vec![0],
        type_arguments: vec![binder(0, 0)],
    };
    let contract = MemberRequirementKey {
        subject: binder(0, 0),
        call: RequiredMemberKey::Method {
            name: "copy".into(),
            type_arguments: vec![Some(binder(0, 0)), None],
        },
        arguments: vec![],
        result: binder(0, 0),
        effects: RequiredEffectsKey {
            is_suspend: false,
            is_unsafe: false,
            is_no_gc: false,
        },
    };
    let body = hir::ExportDefaultBodyV1::try_new(
        vec![
            statement(hir::DefaultStatementKindV1::LocalFunction(local_function)),
            statement(hir::DefaultStatementKindV1::MemberRequirementCall {
                contract: Box::new(contract),
                binding: Box::new(binding),
                arguments: vec![value.clone()],
                result: Box::new(hir::DefaultPatternV1::wildcard()),
            }),
        ],
        value,
    )
    .unwrap();
    let definition = Arc::new(
        hir::ExportDefaultTemplateV1::try_new(
            hir::ExportDefaultTemplateKeyV1::new(CallableTemplateOrigin::GenericFunction(root), 1),
            hir::PersistentLexicalRootV1::GenericFunction(root),
            path(StructuralDefinitionSiteRole::DefaultValue, 1),
            locals,
            body,
            binder(0, 0),
            hir::CanonicalBooleanV1::False,
            hir::CanonicalBinderUseListV1::try_new(vec![binder(0, 0)]).unwrap(),
            hir::OptionalTemplateReceiverV1::Absent,
            hir::CanonicalTemplateValueParametersV1::try_new(vec![
                hir::TemplateValueParameterV1::try_new(0, parameter).unwrap(),
            ])
            .unwrap(),
            hir::ExportDefaultReferenceSetV1::default(),
            origin(),
        )
        .unwrap(),
    );
    let original = serde_json::to_value(BodyJson::new(definition.as_ref())).unwrap();
    for argument in [core_type("Long"), binder(0, 2)] {
        let application = hir::SemanticDefaultApplication::try_new(
            definition.clone(),
            hir::DefaultTemplateProviderShapeV1::try_new(0, 1).unwrap(),
            hir::CanonicalBinderUseListV1::try_new(vec![argument.clone()]).unwrap(),
        )
        .unwrap();
        let applied = serde_json::to_value(applied_default(&application)).unwrap();
        let expected = json!(QueryJson(&argument));
        assert_eq!(applied["result"], expected);
        assert_eq!(applied["body"]["value"]["resultType"], expected);
        assert_eq!(applied["locals"][0]["valueType"], expected);
        assert_eq!(
            applied["body"]["value"]["operation"],
            original["body"]["value"]["operation"]
        );
        assert_eq!(
            applied["previousParameters"],
            original["previousParameters"]
        );
        assert_eq!(applied["origin"], original["origin"]);
        let signature = &applied["body"]["statements"][0]["operation"]["function"]["functionType"];
        assert_eq!(signature["parameters"][1], json!(QueryJson(&binder(0, 0))));
        let expected_outer = match argument {
            SignatureTypeKey::Binder { .. } => json!(QueryJson(&binder(1, 2))),
            _ => expected.clone(),
        };
        assert_eq!(signature["parameters"][0], expected_outer);
        assert_eq!(signature["result"], expected_outer);
        let requirement = &applied["body"]["statements"][1]["operation"];
        assert_eq!(requirement["contract"]["subject"], expected);
        assert_eq!(
            requirement["contract"]["call"]["typeArguments"],
            json!([expected, null])
        );
        assert_eq!(requirement["binding"]["typeArguments"][0], expected);
        assert_eq!(
            requirement["binding"]["body"],
            original["body"]["statements"][1]["operation"]["binding"]["body"]
        );
        assert!(Arc::ptr_eq(application.definition(), &definition));
    }
    assert_eq!(
        serde_json::to_value(BodyJson::new(definition.as_ref())).unwrap(),
        original
    );
}
