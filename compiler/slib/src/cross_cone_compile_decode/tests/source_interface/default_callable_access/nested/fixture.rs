use super::*;
use scoop_identity::{
    GeneratedCallableKey, LexicalCallableParent, PersistentGeneratedCallableId,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

pub(super) fn references(generated_target: bool) -> CallableSourceSurface {
    let base = default_fixture::fixture(default_fixture::Case::Defined);
    let ty = base.interface.default_templates().records()[0]
        .result()
        .clone();
    let mut fixture = default_envelope::local_function_fixture(0, ty.clone(), 0);
    let original = OdrFreeHirFoundation::try_new(fixture.foundation.clone()).unwrap();
    let CallableTemplateOrigin::Function(owner) = fixture.owner else {
        panic!("function")
    };
    let template = &fixture.interface.default_templates().records()[0];
    let local = template
        .body()
        .statements()
        .iter()
        .find_map(|statement| match statement.kind() {
            DefaultStatementKindV1::LocalFunction(local) => Some(local.declaration()),
            _ => None,
        })
        .unwrap();
    let CallableTemplateOrigin::Function(local_id) = local else {
        panic!("nongeneric local")
    };
    let path = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
        [StructuralPathSegment::new(
            StructuralDefinitionSiteRole::CallableConversion,
            0,
        )],
    );
    let invoke = CborIdentityRecord::<PersistentGeneratedCallableId, _>::from_key(
        GeneratedCallableKey::CallableReferenceInvoke {
            parent: LexicalCallableParent::function(owner),
            path: path.clone(),
        },
    )
    .unwrap();
    fixture
        .foundation
        .set_generated_callables(vec![invoke.clone()])
        .unwrap();
    let mut subjects: Vec<_> = fixture
        .interface
        .nominal_interfaces()
        .all_records()
        .map(|record| match record.declaration() {
            SourceNominalId::Concrete(id) => DefinitionOriginSubject::Type(id),
            SourceNominalId::GenericTemplate(id) => DefinitionOriginSubject::GenericType(id),
        })
        .collect();
    subjects.extend([
        DefinitionOriginSubject::Function(owner),
        DefinitionOriginSubject::Function(local_id),
    ]);
    let mut origins: Vec<_> = subjects
        .into_iter()
        .map(|subject| original.definition_origin(subject).unwrap().clone())
        .collect();
    origins.push(DefinitionOriginRecord::new(
        DefinitionOriginSubject::GeneratedCallable(invoke.id()),
        template.definition_origin().origin().clone(),
    ));
    fixture.foundation.set_definition_origins(origins).unwrap();
    let origin = template.definition_origin().clone();
    let function_type = SignatureTypeKey::Function {
        effect: Effect::Ordinary,
        parameters: vec![ty.clone()],
        result: Box::new(ty),
    };
    let mut statements = template.body().statements().to_vec();
    for second in [false, true] {
        let declaration = if generated_target {
            DefaultCallableDeclarationV1::Generated(invoke.id())
        } else {
            DefaultCallableDeclarationV1::Function(if second { owner } else { local_id })
        };
        let callee =
            DefaultCallableRefV1::try_new(declaration, OptionalSignatureType::Absent, vec![])
                .unwrap();
        let target = if !second && !generated_target {
            DefaultCallableReferenceTargetV1::Local {
                declaration: local,
                callee,
            }
        } else {
            DefaultCallableReferenceTargetV1::Named(callee)
        };
        let reference = DefaultCallableReferenceV1::try_new(
            invoke.id(),
            path.clone(),
            target,
            function_type.clone(),
            vec![],
            0,
        )
        .unwrap();
        let expression = DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::CallableReference(reference),
            function_type.clone(),
            origin.clone(),
        )
        .unwrap();
        statements.push(
            DefaultStatementV1::try_new(
                DefaultStatementKindV1::Expr(Box::new(expression)),
                origin.clone(),
            )
            .unwrap(),
        );
    }
    let locals = template.locals().records().to_vec();
    let refs = template.references();
    let mut callables = refs.callables().to_vec();
    callables.push(ExportDefaultReferenceV1::new(
        ExportDefaultCallableTargetV1::CallableReference {
            invoke: invoke.id(),
        },
        origin.clone(),
        ExportDefaultAccessWitnessV1::new(fixture.owner, ExportDefaultCallDomainV1::DirectPublic),
    ));
    let references = ExportDefaultReferenceSetV1::try_new(
        callables,
        refs.constructors().to_vec(),
        refs.types().to_vec(),
        refs.globals().to_vec(),
        refs.singleton_values().to_vec(),
        refs.fields().to_vec(),
    )
    .unwrap();
    default_envelope::support::replace_contents(&mut fixture, locals, statements);
    default_fixture::replace_references(&mut fixture, references);
    fixture
}

pub(super) fn first_reference(template: &ExportDefaultTemplateV1) -> &DefaultCallableReferenceV1 {
    template
        .body()
        .statements()
        .iter()
        .find_map(|statement| match statement.kind() {
            DefaultStatementKindV1::Expr(expression) => match expression.kind() {
                DefaultExpressionKindV1::CallableReference(reference) => Some(reference),
                _ => None,
            },
            _ => None,
        })
        .unwrap()
}

pub(super) fn replace_first_reference(
    fixture: &mut CallableSourceSurface,
    reference: DefaultCallableReferenceV1,
) {
    let template = &fixture.interface.default_templates().records()[0];
    let mut changed = false;
    let statements = template
        .body()
        .statements()
        .iter()
        .map(|statement| {
            if !changed
                && let DefaultStatementKindV1::Expr(expression) = statement.kind()
                && matches!(
                    expression.kind(),
                    DefaultExpressionKindV1::CallableReference(_)
                )
            {
                changed = true;
                return DefaultStatementV1::try_new(
                    DefaultStatementKindV1::Expr(Box::new(
                        DefaultExpressionV1::try_new(
                            DefaultExpressionKindV1::CallableReference(reference.clone()),
                            expression.result_type().clone(),
                            expression.definition_origin().clone(),
                        )
                        .unwrap(),
                    )),
                    statement.definition_origin().clone(),
                )
                .unwrap();
            }
            statement.clone()
        })
        .collect();
    assert!(changed);
    let locals = template.locals().records().to_vec();
    default_envelope::support::replace_contents(fixture, locals, statements);
}
