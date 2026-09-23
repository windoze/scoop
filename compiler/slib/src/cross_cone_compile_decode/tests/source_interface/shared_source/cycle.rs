use super::*;

pub(super) fn install(fixture: &mut CallableSourceSurface) {
    let old = &fixture.interface.default_templates().records()[0];
    let origin = old.definition_origin();
    let value = DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::StructConstruct {
            owner_type: old.result().clone(),
            fields: vec![],
        },
        old.result().clone(),
        origin.clone(),
    )
    .unwrap();
    let CallableTemplateOrigin::Function(id) = fixture.owner else {
        panic!("plain function");
    };
    let callee = DefaultCallableRefV1::try_new(
        DefaultCallableDeclarationV1::Function(id),
        scoop_identity::OptionalSignatureType::Absent,
        vec![],
    )
    .unwrap();
    let result = fixture
        .interface
        .callable_interfaces()
        .declaration(fixture.owner)
        .unwrap()
        .result();
    let call = DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::Call {
            callee: callee.clone(),
            arguments: vec![value.clone()],
        },
        result.clone(),
        origin.clone(),
    )
    .unwrap();
    let witness =
        ExportDefaultAccessWitnessV1::new(fixture.owner, ExportDefaultCallDomainV1::DirectPublic);
    let mut types = vec![old.result().clone(), result.clone()];
    types.sort_unstable();
    types.dedup();
    let references = ExportDefaultReferenceSetV1::try_new(
        vec![ExportDefaultReferenceV1::new(
            ExportDefaultCallableTargetV1::Callable(callee),
            origin.clone(),
            witness.clone(),
        )],
        vec![],
        types
            .into_iter()
            .map(|ty| ExportDefaultReferenceV1::new(ty, origin.clone(), witness.clone()))
            .collect(),
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let template = ExportDefaultTemplateV1::try_new(
        old.key(),
        old.definition_root(),
        old.definition_path().clone(),
        CanonicalTemplateLocalTableV1::try_new(vec![]).unwrap(),
        ExportDefaultBodyV1::try_new(
            vec![
                DefaultStatementV1::try_new(
                    DefaultStatementKindV1::Expr(Box::new(call)),
                    origin.clone(),
                )
                .unwrap(),
            ],
            value,
        )
        .unwrap(),
        old.result().clone(),
        old.allows_suspend(),
        old.type_parameters().clone(),
        old.receiver().clone(),
        old.value_parameters().clone(),
        references,
        origin.clone(),
    )
    .unwrap();
    replace(
        fixture,
        fixture.interface.source_interfaces().clone(),
        CanonicalExportDefaultTemplatesV1::try_new(vec![template]).unwrap(),
    );
}
