use scoop_identity::{
    CallableTemplateOrigin, Effect, GeneratedCallableKey, LexicalCallableParent,
    LexicalCallableRole, OptionalSignatureType, PersistentGeneratedCallableId, SignatureTypeKey,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

use super::*;
use crate::{
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalTemplateLocalTableV1,
    CanonicalTemplateValueParametersV1, DefaultCallableDeclarationV1, DefaultCallableRefV1,
    DefaultCallableReferenceTargetV1, DefaultExpressionKindV1, DefaultExpressionV1,
    DefaultStatementKindV1, DefaultStatementV1, ExportDefaultBodyV1, ExportDefaultReferenceSetV1,
    ExportDefaultTemplateKeyV1, OptionalTemplateReceiverV1, PersistentLexicalRootV1,
};

use super::super::test_support::{Fixture, binder, source_function};

#[test]
fn walks_the_whole_body_and_closes_local_callable_uses() {
    let fixture = Fixture::new();
    let local_function_id = source_function("nestedLocal");
    let local_declaration = CallableTemplateOrigin::Function(local_function_id);
    let root_path = template(&fixture).definition_path().clone();
    let function_type = function(binder(0));

    let local_path = child_path(
        &root_path,
        StructuralDefinitionSiteRole::LocalDeclaration,
        0,
    );
    let local = DefaultLocalFunctionV1::try_new(
        local_declaration,
        local_path.clone(),
        function_type.clone(),
        Vec::new(),
        0,
    )
    .unwrap();

    let lambda_path = child_path(&root_path, StructuralDefinitionSiteRole::Lambda, 0);
    let lambda_body = lexical_body(
        fixture.function,
        LexicalCallableRole::LambdaBody,
        lambda_path.clone(),
    );
    let lambda = DefaultLambdaV1::try_new(
        lambda_body,
        lambda_path.clone(),
        function_type.clone(),
        DefaultCallableBodyTypeArgumentsV1::lexical(),
        Vec::new(),
        0,
    )
    .unwrap();

    let anonymous_path = child_path(&root_path, StructuralDefinitionSiteRole::Lambda, 0);
    let anonymous_body = lexical_body(
        fixture.function,
        LexicalCallableRole::AnonymousFunctionBody,
        anonymous_path.clone(),
    );
    let anonymous = DefaultAnonymousFunctionV1::try_new(
        anonymous_body,
        anonymous_path.clone(),
        function_type.clone(),
        DefaultCallableBodyTypeArgumentsV1::lexical(),
        Vec::new(),
        0,
    )
    .unwrap();

    let reference_path = child_path(
        &root_path,
        StructuralDefinitionSiteRole::CallableConversion,
        0,
    );
    let invoke = callable_reference_invoke(fixture.function, reference_path.clone());
    let reference = DefaultCallableReferenceV1::try_new(
        invoke,
        reference_path.clone(),
        DefaultCallableReferenceTargetV1::Named(callable(fixture.function)),
        function_type.clone(),
        Vec::new(),
        0,
    )
    .unwrap();

    let template = template_with_body(
        &fixture,
        vec![
            statement(&fixture, DefaultStatementKindV1::LocalFunction(local)),
            expression_statement(
                &fixture,
                DefaultExpressionKindV1::Lambda(lambda),
                function_type.clone(),
            ),
            expression_statement(
                &fixture,
                DefaultExpressionKindV1::AnonymousFunction(anonymous),
                function_type.clone(),
            ),
            expression_statement(
                &fixture,
                DefaultExpressionKindV1::CallableReference(reference),
                function_type.clone(),
            ),
        ],
        expression(
            &fixture,
            DefaultExpressionKindV1::LocalFunctionCall {
                declaration: local_declaration,
                callee: callable(local_function_id),
                captures: Vec::new(),
                arguments: vec![unit(&fixture)],
            },
            binder(0),
        ),
    );
    let identities = vec![
        (
            DefaultNestedCallableIdentityV1::LocalFunction(local_declaration),
            identity_shape(
                DefaultNestedCallableProvenanceV1::TemplateLexical,
                local_path,
                0,
                DefaultNestedCallableBodyShapeV1::Absent,
            ),
            abi_shape(function_type.clone(), Vec::new()),
        ),
        (
            DefaultNestedCallableIdentityV1::Lambda(lambda_body),
            identity_shape(
                DefaultNestedCallableProvenanceV1::TemplateLexical,
                lambda_path,
                0,
                DefaultNestedCallableBodyShapeV1::Lexical,
            ),
            abi_shape(function_type.clone(), Vec::new()),
        ),
        (
            DefaultNestedCallableIdentityV1::AnonymousFunction(anonymous_body),
            identity_shape(
                DefaultNestedCallableProvenanceV1::TemplateLexical,
                anonymous_path,
                0,
                DefaultNestedCallableBodyShapeV1::Lexical,
            ),
            abi_shape(function_type.clone(), Vec::new()),
        ),
        (
            DefaultNestedCallableIdentityV1::CallableReference(invoke),
            identity_shape(
                DefaultNestedCallableProvenanceV1::TemplateLexical,
                reference_path,
                0,
                DefaultNestedCallableBodyShapeV1::Absent,
            ),
            abi_shape(function_type, Vec::new()),
        ),
    ];
    let expected = identities
        .iter()
        .flat_map(|(identity, _, _)| {
            [
                (*identity, DefaultNestedCallableAuthorityQueryV1::Identity),
                (*identity, DefaultNestedCallableAuthorityQueryV1::Abi),
            ]
        })
        .collect::<Vec<_>>();
    let mut authority = AuthoritySet::new(identities);

    validate_body(&template, &mut authority).unwrap();

    assert_eq!(authority.observed, expected);
}

#[test]
fn traverses_a_callable_reference_receiver() {
    let fixture = Fixture::new();
    let template_root = template(&fixture).definition_path().clone();
    let function_type = function(binder(0));
    let lambda_path = child_path(&template_root, StructuralDefinitionSiteRole::Lambda, 11);
    let lambda_body = lexical_body(
        fixture.function,
        LexicalCallableRole::LambdaBody,
        lambda_path.clone(),
    );
    let receiver = expression(
        &fixture,
        DefaultExpressionKindV1::Lambda(
            DefaultLambdaV1::try_new(
                lambda_body,
                lambda_path.clone(),
                function_type.clone(),
                DefaultCallableBodyTypeArgumentsV1::lexical(),
                Vec::new(),
                0,
            )
            .unwrap(),
        ),
        function_type.clone(),
    );
    let reference_path = child_path(
        &template_root,
        StructuralDefinitionSiteRole::CallableConversion,
        12,
    );
    let invoke = callable_reference_invoke(fixture.function, reference_path.clone());
    let reference = DefaultCallableReferenceV1::try_new(
        invoke,
        reference_path.clone(),
        DefaultCallableReferenceTargetV1::BoundExtension {
            receiver: Box::new(receiver),
            callee: callable(fixture.function),
        },
        function_type.clone(),
        Vec::new(),
        0,
    )
    .unwrap();
    let template = template_with_body(
        &fixture,
        Vec::new(),
        expression(
            &fixture,
            DefaultExpressionKindV1::CallableReference(reference),
            function_type.clone(),
        ),
    );
    let mut authority = AuthoritySet::new(vec![
        (
            DefaultNestedCallableIdentityV1::CallableReference(invoke),
            identity_shape(
                DefaultNestedCallableProvenanceV1::TemplateLexical,
                reference_path,
                0,
                DefaultNestedCallableBodyShapeV1::Absent,
            ),
            abi_shape(function_type.clone(), Vec::new()),
        ),
        (
            DefaultNestedCallableIdentityV1::Lambda(lambda_body),
            identity_shape(
                DefaultNestedCallableProvenanceV1::TemplateLexical,
                lambda_path,
                0,
                DefaultNestedCallableBodyShapeV1::Lexical,
            ),
            abi_shape(function_type, Vec::new()),
        ),
    ]);

    validate_body(&template, &mut authority).unwrap();

    assert!(authority.observed.iter().any(|(identity, query)| {
        *identity == DefaultNestedCallableIdentityV1::Lambda(lambda_body)
            && *query == DefaultNestedCallableAuthorityQueryV1::Abi
    }));
}

#[test]
fn rejects_missing_and_duplicate_local_function_descriptors() {
    let fixture = Fixture::new();
    let local_function_id = source_function("missingLocal");
    let declaration = CallableTemplateOrigin::Function(local_function_id);
    let missing = template_with_body(
        &fixture,
        Vec::new(),
        expression(
            &fixture,
            DefaultExpressionKindV1::LocalFunctionCall {
                declaration,
                callee: callable(local_function_id),
                captures: Vec::new(),
                arguments: Vec::new(),
            },
            binder(0),
        ),
    );
    assert_eq!(
        validate_body(&missing, &mut AuthoritySet::new(Vec::new())).unwrap_err(),
        DefaultNestedCallableAbiValidationError::MissingLocalFunction {
            declaration,
            site: DefaultNestedCallableLocalUseV1::DirectCall,
        }
    );

    let reference_path = child_path(
        template(&fixture).definition_path(),
        StructuralDefinitionSiteRole::CallableConversion,
        4,
    );
    let invoke = callable_reference_invoke(fixture.function, reference_path.clone());
    let function_type = function(binder(0));
    let reference = DefaultCallableReferenceV1::try_new(
        invoke,
        reference_path.clone(),
        DefaultCallableReferenceTargetV1::Local {
            declaration,
            callee: callable(local_function_id),
        },
        function_type.clone(),
        Vec::new(),
        0,
    )
    .unwrap();
    let missing_reference = template_with_body(
        &fixture,
        Vec::new(),
        expression(
            &fixture,
            DefaultExpressionKindV1::CallableReference(reference),
            function_type.clone(),
        ),
    );
    let mut reference_authority = AuthoritySet::new(vec![(
        DefaultNestedCallableIdentityV1::CallableReference(invoke),
        identity_shape(
            DefaultNestedCallableProvenanceV1::TemplateLexical,
            reference_path,
            0,
            DefaultNestedCallableBodyShapeV1::Absent,
        ),
        abi_shape(function_type, Vec::new()),
    )]);
    assert_eq!(
        validate_body(&missing_reference, &mut reference_authority).unwrap_err(),
        DefaultNestedCallableAbiValidationError::MissingLocalFunction {
            declaration,
            site: DefaultNestedCallableLocalUseV1::CallableReference,
        }
    );

    let local_path = child_path(
        template(&fixture).definition_path(),
        StructuralDefinitionSiteRole::LocalDeclaration,
        3,
    );
    let descriptor = DefaultLocalFunctionV1::try_new(
        declaration,
        local_path.clone(),
        function(binder(0)),
        Vec::new(),
        0,
    )
    .unwrap();
    let duplicate = template_with_body(
        &fixture,
        vec![
            statement(
                &fixture,
                DefaultStatementKindV1::LocalFunction(descriptor.clone()),
            ),
            statement(&fixture, DefaultStatementKindV1::LocalFunction(descriptor)),
        ],
        unit(&fixture),
    );
    let mut authority = AuthoritySet::new(vec![(
        DefaultNestedCallableIdentityV1::LocalFunction(declaration),
        identity_shape(
            DefaultNestedCallableProvenanceV1::TemplateLexical,
            local_path,
            0,
            DefaultNestedCallableBodyShapeV1::Absent,
        ),
        abi_shape(function(binder(0)), Vec::new()),
    )]);
    assert_eq!(
        validate_body(&duplicate, &mut authority).unwrap_err(),
        DefaultNestedCallableAbiValidationError::DuplicateLocalFunction { declaration }
    );
}

#[test]
fn body_walk_uses_the_callers_semantic_depth_budget() {
    let fixture = Fixture::new();
    let template = template(&fixture);
    let path = WirePath::root().field(9);
    let mut meter = BudgetMeter::new(DecodeLimits {
        semantic_recursion: 1,
        ..DecodeLimits::default()
    });
    let error = template
        .body()
        .validate_nested_callable_abi_semantics(
            &template,
            &mut AuthoritySet::new(Vec::new()),
            &mut meter,
            &path,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        DefaultNestedCallableAbiValidationError::Resource(ref error)
            if error.kind()
                == &WireErrorKind::LimitExceeded {
                    resource: ResourceKind::SemanticRecursion,
                    limit: 1,
                    observed: 2,
                }
                && error.path() == &path
    ));
}

#[test]
fn validates_all_four_nested_callable_descriptor_kinds() {
    let fixture = Fixture::new();
    let template = template(&fixture);
    let value_type = binder(0);
    let function_type = function(value_type.clone());
    let capture = fixture.capture(0);

    let local_path = child_path(
        template.definition_path(),
        StructuralDefinitionSiteRole::LocalDeclaration,
        0,
    );
    let local = DefaultLocalFunctionV1::try_new(
        CallableTemplateOrigin::Function(fixture.function),
        local_path.clone(),
        function_type.clone(),
        vec![capture.clone()],
        1,
    )
    .unwrap();
    validate(
        &local,
        &template,
        Authority::new(
            DefaultNestedCallableIdentityV1::LocalFunction(CallableTemplateOrigin::Function(
                fixture.function,
            )),
            identity_shape(
                DefaultNestedCallableProvenanceV1::TemplateLexical,
                local_path,
                1,
                DefaultNestedCallableBodyShapeV1::Absent,
            ),
            abi_shape(function_type.clone(), vec![value_type.clone()]),
        ),
    )
    .unwrap();

    let lambda_path = child_path(
        template.definition_path(),
        StructuralDefinitionSiteRole::Lambda,
        0,
    );
    let lambda_body = lexical_body(
        fixture.function,
        LexicalCallableRole::LambdaBody,
        lambda_path.clone(),
    );
    let lambda = DefaultLambdaV1::try_new(
        lambda_body,
        lambda_path.clone(),
        function_type.clone(),
        DefaultCallableBodyTypeArgumentsV1::lexical(),
        vec![capture.clone()],
        1,
    )
    .unwrap();
    validate(
        &lambda,
        &template,
        Authority::new(
            DefaultNestedCallableIdentityV1::Lambda(lambda_body),
            identity_shape(
                DefaultNestedCallableProvenanceV1::TemplateLexical,
                lambda_path,
                1,
                DefaultNestedCallableBodyShapeV1::Lexical,
            ),
            abi_shape(function_type.clone(), vec![value_type.clone()]),
        ),
    )
    .unwrap();

    let dependency_path = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 9),
        [StructuralPathSegment::new(
            StructuralDefinitionSiteRole::Lambda,
            2,
        )],
    );
    let anonymous_body = lexical_body(
        fixture.function,
        LexicalCallableRole::AnonymousFunctionBody,
        dependency_path.clone(),
    );
    let anonymous = DefaultAnonymousFunctionV1::try_new(
        anonymous_body,
        dependency_path.clone(),
        function_type.clone(),
        DefaultCallableBodyTypeArgumentsV1::try_explicit(vec![value_type.clone()]).unwrap(),
        vec![capture.clone()],
        1,
    )
    .unwrap();
    validate(
        &anonymous,
        &template,
        Authority::new(
            DefaultNestedCallableIdentityV1::AnonymousFunction(anonymous_body),
            identity_shape(
                DefaultNestedCallableProvenanceV1::DefaultDependency,
                dependency_path,
                1,
                DefaultNestedCallableBodyShapeV1::Explicit { arity: 1 },
            ),
            abi_shape(function_type.clone(), vec![value_type.clone()]),
        ),
    )
    .unwrap();

    let reference_path = child_path(
        template.definition_path(),
        StructuralDefinitionSiteRole::CallableConversion,
        0,
    );
    let invoke =
        PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::CallableReferenceInvoke {
            parent: LexicalCallableParent::function(fixture.function),
            path: reference_path.clone(),
        })
        .unwrap();
    let reference = DefaultCallableReferenceV1::try_new(
        invoke,
        reference_path.clone(),
        DefaultCallableReferenceTargetV1::Named(
            DefaultCallableRefV1::try_new(
                DefaultCallableDeclarationV1::Function(fixture.function),
                OptionalSignatureType::Absent,
                Vec::new(),
            )
            .unwrap(),
        ),
        function_type.clone(),
        vec![capture],
        1,
    )
    .unwrap();
    validate(
        &reference,
        &template,
        Authority::new(
            DefaultNestedCallableIdentityV1::CallableReference(invoke),
            identity_shape(
                DefaultNestedCallableProvenanceV1::TemplateLexical,
                reference_path,
                1,
                DefaultNestedCallableBodyShapeV1::Absent,
            ),
            abi_shape(function_type, vec![value_type]),
        ),
    )
    .unwrap();
}

#[test]
fn rejects_identity_path_and_template_ownership_mismatches() {
    let fixture = Fixture::new();
    let template = template(&fixture);
    let descriptor_path = child_path(
        template.definition_path(),
        StructuralDefinitionSiteRole::Lambda,
        1,
    );
    let body = lexical_body(
        fixture.function,
        LexicalCallableRole::LambdaBody,
        descriptor_path.clone(),
    );
    let lambda = DefaultLambdaV1::try_new(
        body,
        descriptor_path.clone(),
        function(binder(0)),
        DefaultCallableBodyTypeArgumentsV1::lexical(),
        Vec::new(),
        0,
    )
    .unwrap();
    let wrong_path = child_path(
        template.definition_path(),
        StructuralDefinitionSiteRole::Lambda,
        2,
    );
    let error = validate(
        &lambda,
        &template,
        Authority::new(
            DefaultNestedCallableIdentityV1::Lambda(body),
            identity_shape(
                DefaultNestedCallableProvenanceV1::TemplateLexical,
                wrong_path.clone(),
                0,
                DefaultNestedCallableBodyShapeV1::Lexical,
            ),
            abi_shape(function(binder(0)), Vec::new()),
        ),
    )
    .unwrap_err();
    assert_eq!(
        error,
        DefaultNestedCallableAbiValidationError::DefinitionPath {
            kind: DefaultNestedCallableKindV1::Lambda,
            expected: wrong_path,
            actual: descriptor_path,
        }
    );

    let unrelated = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 7),
        [],
    );
    let lambda = DefaultLambdaV1::try_new(
        body,
        unrelated.clone(),
        function(binder(0)),
        DefaultCallableBodyTypeArgumentsV1::lexical(),
        Vec::new(),
        0,
    )
    .unwrap();
    let error = validate(
        &lambda,
        &template,
        Authority::new(
            DefaultNestedCallableIdentityV1::Lambda(body),
            identity_shape(
                DefaultNestedCallableProvenanceV1::TemplateLexical,
                unrelated.clone(),
                0,
                DefaultNestedCallableBodyShapeV1::Lexical,
            ),
            abi_shape(function(binder(0)), Vec::new()),
        ),
    )
    .unwrap_err();
    assert_eq!(
        error,
        DefaultNestedCallableAbiValidationError::DefinitionPathNotDescendant {
            kind: DefaultNestedCallableKindV1::Lambda,
            template: template.definition_path().clone(),
            actual: unrelated,
        }
    );
}

#[test]
fn rejects_body_argument_owner_signature_and_capture_mismatches() {
    let fixture = Fixture::new();
    let template = template(&fixture);
    let descriptor_path = child_path(
        template.definition_path(),
        StructuralDefinitionSiteRole::Lambda,
        3,
    );
    let body = lexical_body(
        fixture.function,
        LexicalCallableRole::LambdaBody,
        descriptor_path.clone(),
    );
    let actual_function = function(binder(0));
    let lambda = DefaultLambdaV1::try_new(
        body,
        descriptor_path.clone(),
        actual_function.clone(),
        DefaultCallableBodyTypeArgumentsV1::try_explicit(vec![binder(0)]).unwrap(),
        vec![fixture.capture(0)],
        2,
    )
    .unwrap();

    let make_authority = |body_shape, owner, function_type, captures| {
        Authority::new(
            DefaultNestedCallableIdentityV1::Lambda(body),
            identity_shape(
                DefaultNestedCallableProvenanceV1::TemplateLexical,
                descriptor_path.clone(),
                owner,
                body_shape,
            ),
            abi_shape(function_type, captures),
        )
    };

    assert!(matches!(
        validate(
            &lambda,
            &template,
            make_authority(
                DefaultNestedCallableBodyShapeV1::Lexical,
                2,
                actual_function.clone(),
                vec![binder(0)]
            ),
        ),
        Err(DefaultNestedCallableAbiValidationError::BodyArguments {
            kind: DefaultNestedCallableKindV1::Lambda,
            expected: DefaultNestedCallableBodyShapeV1::Lexical,
            actual: DefaultNestedCallableBodyShapeV1::Explicit { arity: 1 },
        })
    ));
    assert!(matches!(
        validate(
            &lambda,
            &template,
            make_authority(
                DefaultNestedCallableBodyShapeV1::Explicit { arity: 1 },
                1,
                actual_function.clone(),
                vec![binder(0)]
            ),
        ),
        Err(
            DefaultNestedCallableAbiValidationError::OwnerTypeParameterCount {
                expected: 1,
                actual: 2,
                ..
            }
        )
    ));
    assert!(matches!(
        validate(
            &lambda,
            &template,
            make_authority(
                DefaultNestedCallableBodyShapeV1::Explicit { arity: 1 },
                2,
                function(binder(7)),
                vec![binder(0)]
            ),
        ),
        Err(DefaultNestedCallableAbiValidationError::FunctionType { .. })
    ));
    assert!(matches!(
        validate(
            &lambda,
            &template,
            make_authority(
                DefaultNestedCallableBodyShapeV1::Explicit { arity: 1 },
                2,
                actual_function.clone(),
                Vec::new()
            ),
        ),
        Err(DefaultNestedCallableAbiValidationError::CaptureArity {
            expected: 0,
            actual: 1,
            ..
        })
    ));
    assert!(matches!(
        validate(
            &lambda,
            &template,
            make_authority(
                DefaultNestedCallableBodyShapeV1::Explicit { arity: 1 },
                2,
                actual_function,
                vec![binder(9)]
            ),
        ),
        Err(DefaultNestedCallableAbiValidationError::CaptureType { index: 0, .. })
    ));
}

#[test]
fn preserves_authority_query_and_resource_failures() {
    let fixture = Fixture::new();
    let template = template(&fixture);
    let descriptor_path = child_path(
        template.definition_path(),
        StructuralDefinitionSiteRole::LocalDeclaration,
        0,
    );
    let local = DefaultLocalFunctionV1::try_new(
        CallableTemplateOrigin::Function(fixture.function),
        descriptor_path.clone(),
        function(binder(0)),
        Vec::new(),
        0,
    )
    .unwrap();
    let identity = DefaultNestedCallableIdentityV1::LocalFunction(
        CallableTemplateOrigin::Function(fixture.function),
    );
    let mut authority = Authority::new(
        identity,
        identity_shape(
            DefaultNestedCallableProvenanceV1::TemplateLexical,
            descriptor_path,
            0,
            DefaultNestedCallableBodyShapeV1::Absent,
        ),
        abi_shape(function(binder(0)), Vec::new()),
    );
    authority.failure = Some(DefaultNestedCallableAuthorityQueryV1::Abi);
    assert_eq!(
        validate(&local, &template, authority).unwrap_err(),
        DefaultNestedCallableAbiValidationError::Authority {
            kind: DefaultNestedCallableKindV1::LocalFunction,
            query: DefaultNestedCallableAuthorityQueryV1::Abi,
            error: AuthorityError,
        }
    );

    let path = WirePath::root().field(4);
    let mut meter = BudgetMeter::new(DecodeLimits {
        semantic_recursion: 0,
        ..DecodeLimits::default()
    });
    let mut authority = Authority::new(
        identity,
        identity_shape(
            DefaultNestedCallableProvenanceV1::TemplateLexical,
            local.definition_path().clone(),
            0,
            DefaultNestedCallableBodyShapeV1::Absent,
        ),
        abi_shape(function(binder(0)), Vec::new()),
    );
    let error = local
        .validate_nested_callable_abi_semantics(&template, &mut authority, &mut meter, &path)
        .unwrap_err();
    assert!(matches!(
        error,
        DefaultNestedCallableAbiValidationError::Resource(ref error)
            if error.kind()
                == &WireErrorKind::LimitExceeded {
                    resource: ResourceKind::SemanticRecursion,
                    limit: 0,
                    observed: 1,
                }
                && error.path() == &path
    ));
}

trait ValidateDescriptor {
    fn validate<A, E>(
        &self,
        template: &ExportDefaultTemplateV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>>
    where
        A: DefaultNestedCallableSemanticAuthority<E>;
}

impl ValidateDescriptor for DefaultLocalFunctionV1 {
    fn validate<A, E>(
        &self,
        template: &ExportDefaultTemplateV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>>
    where
        A: DefaultNestedCallableSemanticAuthority<E>,
    {
        self.validate_nested_callable_abi_semantics(template, authority, meter, path)
    }
}

impl ValidateDescriptor for DefaultLambdaV1 {
    fn validate<A, E>(
        &self,
        template: &ExportDefaultTemplateV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>>
    where
        A: DefaultNestedCallableSemanticAuthority<E>,
    {
        self.validate_nested_callable_abi_semantics(template, authority, meter, path)
    }
}

impl ValidateDescriptor for DefaultAnonymousFunctionV1 {
    fn validate<A, E>(
        &self,
        template: &ExportDefaultTemplateV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>>
    where
        A: DefaultNestedCallableSemanticAuthority<E>,
    {
        self.validate_nested_callable_abi_semantics(template, authority, meter, path)
    }
}

impl ValidateDescriptor for DefaultCallableReferenceV1 {
    fn validate<A, E>(
        &self,
        template: &ExportDefaultTemplateV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>>
    where
        A: DefaultNestedCallableSemanticAuthority<E>,
    {
        self.validate_nested_callable_abi_semantics(template, authority, meter, path)
    }
}

fn validate<T: ValidateDescriptor>(
    descriptor: &T,
    template: &ExportDefaultTemplateV1,
    mut authority: Authority,
) -> Result<(), DefaultNestedCallableAbiValidationError<AuthorityError>> {
    descriptor.validate(
        template,
        &mut authority,
        &mut BudgetMeter::new(DecodeLimits::default()),
        &WirePath::root().field(5),
    )
}

fn template(fixture: &Fixture) -> ExportDefaultTemplateV1 {
    template_with_body(fixture, Vec::new(), unit(fixture))
}

fn template_with_body(
    fixture: &Fixture,
    statements: Vec<DefaultStatementV1>,
    value: DefaultExpressionV1,
) -> ExportDefaultTemplateV1 {
    let result = value.result_type().clone();
    let origin = origin(fixture);
    ExportDefaultTemplateV1::try_new(
        ExportDefaultTemplateKeyV1::new(CallableTemplateOrigin::Function(fixture.function), 0),
        PersistentLexicalRootV1::Function(fixture.function),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
            [],
        ),
        CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        ExportDefaultBodyV1::try_new(statements, value).unwrap(),
        result,
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(Vec::new()).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(Vec::new()).unwrap(),
        ExportDefaultReferenceSetV1::default(),
        origin,
    )
    .unwrap()
}

fn expression(
    fixture: &Fixture,
    kind: DefaultExpressionKindV1,
    result_type: SignatureTypeKey,
) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(kind, result_type, origin(fixture)).unwrap()
}

fn unit(fixture: &Fixture) -> DefaultExpressionV1 {
    expression(fixture, DefaultExpressionKindV1::UnitLiteral, binder(0))
}

fn statement(fixture: &Fixture, kind: DefaultStatementKindV1) -> DefaultStatementV1 {
    DefaultStatementV1::try_new(kind, origin(fixture)).unwrap()
}

fn expression_statement(
    fixture: &Fixture,
    kind: DefaultExpressionKindV1,
    result_type: SignatureTypeKey,
) -> DefaultStatementV1 {
    statement(
        fixture,
        DefaultStatementKindV1::Expr(Box::new(expression(fixture, kind, result_type))),
    )
}

fn origin(fixture: &Fixture) -> crate::ExportDefinitionSourceV1 {
    fixture.capture(0).first_use_origin().clone()
}

fn callable(declaration: scoop_identity::PersistentFunctionId) -> DefaultCallableRefV1 {
    DefaultCallableRefV1::try_new(
        DefaultCallableDeclarationV1::Function(declaration),
        OptionalSignatureType::Absent,
        Vec::new(),
    )
    .unwrap()
}

fn callable_reference_invoke(
    parent: scoop_identity::PersistentFunctionId,
    path: StructuralDefinitionPath,
) -> PersistentGeneratedCallableId {
    PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::CallableReferenceInvoke {
        parent: LexicalCallableParent::function(parent),
        path,
    })
    .unwrap()
}

fn validate_body(
    template: &ExportDefaultTemplateV1,
    authority: &mut AuthoritySet,
) -> Result<(), DefaultNestedCallableAbiValidationError<AuthorityError>> {
    template.body().validate_nested_callable_abi_semantics(
        template,
        authority,
        &mut BudgetMeter::new(DecodeLimits::default()),
        &WirePath::root().field(8),
    )
}

fn child_path(
    parent: &StructuralDefinitionPath,
    role: StructuralDefinitionSiteRole,
    ordinal: u32,
) -> StructuralDefinitionPath {
    StructuralDefinitionPath::new(
        parent
            .segments()
            .iter()
            .copied()
            .chain([StructuralPathSegment::new(role, ordinal)])
            .collect(),
    )
    .unwrap()
}

fn lexical_body(
    parent: scoop_identity::PersistentFunctionId,
    role: LexicalCallableRole,
    path: StructuralDefinitionPath,
) -> PersistentGeneratedCallableId {
    PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Lexical {
        parent: LexicalCallableParent::function(parent),
        role,
        path,
    })
    .unwrap()
}

fn function(value: SignatureTypeKey) -> SignatureTypeKey {
    SignatureTypeKey::Function {
        effect: Effect::Ordinary,
        parameters: vec![value.clone()],
        result: Box::new(value),
    }
}

const fn identity_shape(
    provenance: DefaultNestedCallableProvenanceV1,
    definition_path: StructuralDefinitionPath,
    owner_type_parameter_count: u32,
    body: DefaultNestedCallableBodyShapeV1,
) -> DefaultNestedCallableIdentityShapeV1 {
    DefaultNestedCallableIdentityShapeV1::new(
        provenance,
        definition_path,
        owner_type_parameter_count,
        body,
    )
}

const fn abi_shape(
    function_type: SignatureTypeKey,
    capture_types: Vec<SignatureTypeKey>,
) -> DefaultNestedCallableAbiShapeV1 {
    DefaultNestedCallableAbiShapeV1::new(function_type, capture_types)
}

struct Authority {
    identity: DefaultNestedCallableIdentityV1,
    identity_shape: DefaultNestedCallableIdentityShapeV1,
    abi_shape: DefaultNestedCallableAbiShapeV1,
    failure: Option<DefaultNestedCallableAuthorityQueryV1>,
}

impl Authority {
    const fn new(
        identity: DefaultNestedCallableIdentityV1,
        identity_shape: DefaultNestedCallableIdentityShapeV1,
        abi_shape: DefaultNestedCallableAbiShapeV1,
    ) -> Self {
        Self {
            identity,
            identity_shape,
            abi_shape,
            failure: None,
        }
    }
}

impl DefaultNestedCallableSemanticAuthority<AuthorityError> for Authority {
    fn default_nested_callable_identity_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, AuthorityError> {
        if self.failure == Some(DefaultNestedCallableAuthorityQueryV1::Identity)
            || identity != self.identity
        {
            Err(AuthorityError)
        } else {
            Ok(self.identity_shape.clone())
        }
    }

    fn default_nested_callable_abi_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        _body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
    ) -> Result<DefaultNestedCallableAbiShapeV1, AuthorityError> {
        if self.failure == Some(DefaultNestedCallableAuthorityQueryV1::Abi)
            || identity != self.identity
        {
            Err(AuthorityError)
        } else {
            Ok(self.abi_shape.clone())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthorityError;

impl fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("nested callable authority rejected the fixture")
    }
}

impl std::error::Error for AuthorityError {}

struct AuthoritySet {
    entries: Vec<(
        DefaultNestedCallableIdentityV1,
        DefaultNestedCallableIdentityShapeV1,
        DefaultNestedCallableAbiShapeV1,
    )>,
    observed: Vec<(
        DefaultNestedCallableIdentityV1,
        DefaultNestedCallableAuthorityQueryV1,
    )>,
}

impl AuthoritySet {
    const fn new(
        entries: Vec<(
            DefaultNestedCallableIdentityV1,
            DefaultNestedCallableIdentityShapeV1,
            DefaultNestedCallableAbiShapeV1,
        )>,
    ) -> Self {
        Self {
            entries,
            observed: Vec::new(),
        }
    }

    fn entry(
        &self,
        identity: DefaultNestedCallableIdentityV1,
    ) -> Option<&(
        DefaultNestedCallableIdentityV1,
        DefaultNestedCallableIdentityShapeV1,
        DefaultNestedCallableAbiShapeV1,
    )> {
        self.entries
            .iter()
            .find(|(candidate, _, _)| *candidate == identity)
    }
}

impl DefaultNestedCallableSemanticAuthority<AuthorityError> for AuthoritySet {
    fn default_nested_callable_identity_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, AuthorityError> {
        let shape = self
            .entry(identity)
            .map(|(_, shape, _)| shape.clone())
            .ok_or(AuthorityError)?;
        self.observed
            .push((identity, DefaultNestedCallableAuthorityQueryV1::Identity));
        Ok(shape)
    }

    fn default_nested_callable_abi_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        _body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
    ) -> Result<DefaultNestedCallableAbiShapeV1, AuthorityError> {
        let shape = self
            .entry(identity)
            .map(|(_, _, shape)| shape.clone())
            .ok_or(AuthorityError)?;
        self.observed
            .push((identity, DefaultNestedCallableAuthorityQueryV1::Abi));
        Ok(shape)
    }
}
