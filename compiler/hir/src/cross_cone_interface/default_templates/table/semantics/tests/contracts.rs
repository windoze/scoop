use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOrigin,
    DefinitionOwnerChain, Effect, GcEffect, GeneratedCallableKey, LexicalCallableParent,
    LexicalCallableRole, LocalValueSelector, OptionalSignatureType, PackagePath, PersistentFieldId,
    PersistentFunctionId, PersistentGenericTypeId, PersistentObjectValueId, PersistentPropertyId,
    PersistentTypeId, SignatureTypeKey, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

use super::*;
use crate::{
    CallableDeclarationIdentityShapeV1, CallableImplementationV1, CallableInfixV1,
    CallableInterfaceRecordV1, CallableInterfaceSemanticAuthority, CallableModalityV1,
    CallableOperatorRoleV1, CallableParameterCallingV1, CallableSafetyV1, CallableSourceEffectsV1,
    CallableSourceParameterV1, CanonicalBinderListV1, CanonicalBinderUseListV1, CanonicalBooleanV1,
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalCallableSourceParametersV1, CanonicalSourceParameterShapesV1,
    CanonicalTemplateLocalTableV1, CanonicalTemplateValueParametersV1, DefaultBodyOperationV1,
    DefaultBodyProviderTypeSiteV1, DefaultCallableBodyTypeArgumentsV1,
    DefaultCallableDeclarationV1, DefaultCallableRefV1, DefaultConstructorRefV1,
    DefaultCoreApplicationV1, DefaultExpressionKindV1, DefaultExpressionV1, DefaultFieldRefV1,
    DefaultLambdaV1, DefaultLocalDataFlowLocalError, DefaultLocalDataFlowSemanticAuthority,
    DefaultLocalDataFlowSiteV1, DefaultNestedCallableAbiShapeV1,
    DefaultNestedCallableAbiValidationError, DefaultNestedCallableAuthorityQueryV1,
    DefaultNestedCallableBodyArgumentsV1, DefaultNestedCallableIdentityShapeV1,
    DefaultNestedCallableIdentityV1, DefaultNestedCallableKindV1,
    DefaultNestedCallableSemanticAuthority, DefaultOperationCoreTypeV1,
    DefaultOperationEntityShapeV1, DefaultOperationEntityV1, DefaultOperationIntrinsicV1,
    DefaultOperationTypeRelationV1, DefaultOperationTypingSemanticAuthority,
    DefaultOperationValueRoleV1, DefaultReferenceSemanticAuthority, DefaultStatementKindV1,
    DefaultStatementOperationV1, DefaultStatementV1, DefaultTemplateOriginSemanticAuthority,
    DefaultTemplateProviderShapeV1, DefaultTemplateRootSemanticAuthority,
    ExportDefaultAccessWitnessV1, ExportDefaultBodyOperationTypingValidationError,
    ExportDefaultBodyV1, ExportDefaultCallDomainV1, ExportDefaultCallableTargetV1,
    ExportDefaultLocalDataFlowValidationError, ExportDefaultReferenceClosureValidationError,
    ExportDefaultReferenceKindV1, ExportDefaultReferenceOccurrenceSiteV1,
    ExportDefaultReferenceSetSemanticValidationError, ExportDefaultReferenceSetV1,
    ExportDefaultReferenceV1, ExportDefaultReferenceValidationError, ExportDefaultTemplateKeyV1,
    ExportDefaultTemplateOriginSemanticValidationError, ExportDefaultTemplateV1,
    ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceV1,
    NominalInterfaceShapeAuthority, OptionalDefaultStatementListV1, OptionalTemplateReceiverV1,
    PersistentLexicalRootV1, PublicDeclarationOwnerV1, PublicLookupAccessV1, PublicNominalKindV1,
    PublicNominalShapeV1, SignatureBinderScopeError, SignatureTypeSemanticError,
    SourceParameterShapeV1, TemplateLocalDefinitionV1, TemplateLocalRecordV1,
};

#[test]
fn validates_every_envelope_and_the_exact_source_closure() {
    let fixture = Fixture::new();
    let templates = fixture.templates(CanonicalBooleanV1::False);
    let callables = fixture.callables(Effect::Ordinary);
    let sources = fixture.sources(true);
    let mut authority = fixture.authority();

    assert_eq!(
        validate_envelopes(&templates, &callables, &sources, &mut authority),
        Ok(())
    );
    assert_eq!(authority.definition_source_validations, 3);
    assert_eq!(authority.template_origin_validations, 1);
}

#[test]
fn requires_matching_callable_and_source_interfaces() {
    let fixture = Fixture::new();
    let templates = fixture.templates(CanonicalBooleanV1::False);
    let empty_callables = CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap();
    assert_eq!(
        validate_envelopes(
            &templates,
            &empty_callables,
            &fixture.sources(true),
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::MissingCallable {
                index: 0,
                owner: fixture.owner,
            }
        )
    );

    let empty_sources = CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap();
    assert_eq!(
        validate_envelopes(
            &templates,
            &fixture.callables(Effect::Ordinary),
            &empty_sources,
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::MissingSource {
                index: 0,
                owner: fixture.owner,
            }
        )
    );
}

#[test]
fn routes_record_contract_and_origin_failures_with_table_identity() {
    let fixture = Fixture::new();
    let templates = fixture.templates(CanonicalBooleanV1::False);
    let callables = fixture.callables(Effect::Suspend);
    let sources = fixture.sources(true);

    assert_eq!(
        validate_envelopes(&templates, &callables, &sources, &mut fixture.authority(),),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::Contract {
                index: 0,
                key: fixture.key,
                error: Box::new(
                    ExportDefaultTemplateContractSemanticValidationError::SuspendPermission {
                        expected: CanonicalBooleanV1::True,
                        actual: CanonicalBooleanV1::False,
                    }
                ),
            }
        )
    );

    let mut authority = fixture.authority();
    authority.reject_template_origin = true;
    assert_eq!(
        validate_envelopes(
            &templates,
            &fixture.callables(Effect::Ordinary),
            &sources,
            &mut authority,
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::Origin {
                index: 0,
                key: fixture.key,
                error: Box::new(
                    ExportDefaultTemplateOriginSemanticValidationError::DefinitionRelation(
                        AuthorityError::TemplateOrigin
                    )
                ),
            }
        )
    );
}

#[test]
fn routes_reference_envelope_failures_with_table_identity() {
    let fixture = Fixture::new();
    let target = ExportDefaultCallableTargetV1::Callable(
        DefaultCallableRefV1::try_new(
            DefaultCallableDeclarationV1::Function(fixture.function),
            OptionalSignatureType::Absent,
            Vec::new(),
        )
        .unwrap(),
    );
    let wrong_domain = fixture.templates_with_references(reference_set(
        &fixture,
        target.clone(),
        ExportDefaultCallDomainV1::DirectAndPublicSlot,
    ));

    assert_eq!(
        validate_envelopes(
            &wrong_domain,
            &fixture.callables(Effect::Ordinary),
            &fixture.sources(true),
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::References {
                index: 0,
                key: fixture.key,
                error: Box::new(ExportDefaultReferenceSetSemanticValidationError::Record {
                    kind: ExportDefaultReferenceKindV1::Callable,
                    index: 0,
                    error: Box::new(ExportDefaultReferenceValidationError::CallDomain {
                        expected: ExportDefaultCallDomainV1::DirectPublic,
                        actual: ExportDefaultCallDomainV1::DirectAndPublicSlot,
                    }),
                }),
            }
        )
    );

    let references = fixture.templates_with_references(reference_set(
        &fixture,
        target,
        ExportDefaultCallDomainV1::DirectPublic,
    ));
    let mut authority = fixture.authority();
    authority.reject_reference_target = true;
    assert_eq!(
        validate_envelopes(
            &references,
            &fixture.callables(Effect::Ordinary),
            &fixture.sources(true),
            &mut authority,
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::References {
                index: 0,
                key: fixture.key,
                error: Box::new(ExportDefaultReferenceSetSemanticValidationError::Record {
                    kind: ExportDefaultReferenceKindV1::Callable,
                    index: 0,
                    error: Box::new(ExportDefaultReferenceValidationError::Target(
                        AuthorityError::ReferenceTarget,
                    )),
                }),
            }
        )
    );
}

#[test]
fn routes_reference_closure_failures_with_table_identity() {
    let fixture = Fixture::new();
    let body = ExportDefaultBodyV1::try_new(
        Vec::new(),
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::UnitLiteral,
            fixture.value_type.clone(),
            fixture.origin.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let templates = fixture.templates_with_body_and_references(
        body,
        CanonicalBooleanV1::False,
        ExportDefaultReferenceSetV1::default(),
    );

    assert_eq!(
        validate_envelopes(
            &templates,
            &fixture.callables(Effect::Ordinary),
            &fixture.sources(true),
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::ReferenceClosure {
                index: 0,
                key: fixture.key,
                error: Box::new(ExportDefaultReferenceClosureValidationError::Missing {
                    kind: ExportDefaultReferenceKindV1::Type,
                    site: ExportDefaultReferenceOccurrenceSiteV1::BodyType(
                        DefaultBodyProviderTypeSiteV1::ExpressionResult,
                    ),
                    insertion_index: 0,
                    definition_origin: Box::new(fixture.origin.clone()),
                }),
            }
        )
    );
}

#[test]
fn routes_local_data_flow_failures_with_table_identity() {
    let fixture = Fixture::new();
    let selector = LocalValueSelector::LocalDeclaration {
        path: StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
            [StructuralPathSegment::new(
                StructuralDefinitionSiteRole::LocalDeclaration,
                0,
            )],
        ),
    };
    let locals = CanonicalTemplateLocalTableV1::try_new(vec![
        TemplateLocalRecordV1::try_new(
            selector.clone(),
            fixture.value_type.clone(),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Source(fixture.origin.clone()),
        )
        .unwrap(),
    ])
    .unwrap();
    let body = ExportDefaultBodyV1::try_new(
        Vec::new(),
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::Local(selector.clone()),
            fixture.value_type.clone(),
            fixture.origin.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let templates = fixture.templates_with_locals_body_and_references(
        locals,
        body,
        CanonicalBooleanV1::False,
        fixture.value_type_references(),
    );

    assert_eq!(
        validate_envelopes(
            &templates,
            &fixture.callables(Effect::Ordinary),
            &fixture.sources(true),
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::LocalDataFlow {
                index: 0,
                key: fixture.key,
                error: Box::new(ExportDefaultLocalDataFlowValidationError::Local {
                    site: DefaultLocalDataFlowSiteV1::Expression,
                    selector: Box::new(selector),
                    error: DefaultLocalDataFlowLocalError::UseBeforeDefinition,
                }),
            }
        )
    );
}

#[test]
fn rejects_source_defaults_missing_from_the_template_table() {
    let fixture = Fixture::new();
    let templates = CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap();

    assert_eq!(
        validate_envelopes(
            &templates,
            &fixture.callables(Effect::Ordinary),
            &fixture.sources(true),
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::SourceClosure(
                ExportDefaultTemplateSourceClosureValidationError::MissingTemplate {
                    source_index: 0,
                    owner: fixture.owner,
                    parameter_position: 0,
                    key: fixture.key,
                }
            )
        )
    );
}

#[test]
fn routes_operation_typing_failures_with_table_identity() {
    let fixture = Fixture::new();
    let condition = DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::UnitLiteral,
        fixture.value_type.clone(),
        fixture.origin.clone(),
    )
    .unwrap();
    let statement = DefaultStatementV1::try_new(
        DefaultStatementKindV1::If {
            condition: Box::new(condition),
            then_body: Vec::new(),
            else_body: OptionalDefaultStatementListV1::absent(),
        },
        fixture.origin.clone(),
    )
    .unwrap();
    let body = ExportDefaultBodyV1::try_new(
        vec![statement],
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::UnitLiteral,
            fixture.value_type.clone(),
            fixture.origin.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let templates = fixture.templates_with_body(body, CanonicalBooleanV1::False);

    assert!(matches!(
        validate_envelopes(
            &templates,
            &fixture.callables(Effect::Ordinary),
            &fixture.sources(true),
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::OperationTyping {
                index: 0,
                key,
                error,
            }
        ) if key == fixture.key
            && matches!(
                error.as_ref(),
                ExportDefaultBodyOperationTypingValidationError::Type { site, .. }
                    if site.operation()
                        == DefaultBodyOperationV1::Statement(
                            DefaultStatementOperationV1::IfCondition,
                        )
                        && site.role() == DefaultOperationValueRoleV1::Condition
            )
    ));
}

#[test]
fn routes_nested_callable_abi_failures_before_reference_validation() {
    let fixture = Fixture::new();
    let lambda_path = StructuralDefinitionPath::new(
        fixture
            .definition_path
            .segments()
            .iter()
            .copied()
            .chain([StructuralPathSegment::new(
                StructuralDefinitionSiteRole::Lambda,
                0,
            )])
            .collect(),
    )
    .unwrap();
    let lambda_body =
        scoop_identity::PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Lexical {
            parent: LexicalCallableParent::function(fixture.function),
            role: LexicalCallableRole::LambdaBody,
            path: lambda_path.clone(),
        })
        .unwrap();
    let function_type = SignatureTypeKey::Function {
        effect: Effect::Ordinary,
        parameters: Vec::new(),
        result: Box::new(fixture.value_type.clone()),
    };
    let lambda = DefaultLambdaV1::try_new(
        lambda_body,
        lambda_path,
        function_type.clone(),
        DefaultCallableBodyTypeArgumentsV1::lexical(),
        Vec::new(),
        0,
    )
    .unwrap();
    let statement = DefaultStatementV1::try_new(
        DefaultStatementKindV1::Expr(Box::new(
            DefaultExpressionV1::try_new(
                DefaultExpressionKindV1::Lambda(lambda),
                function_type,
                fixture.origin.clone(),
            )
            .unwrap(),
        )),
        fixture.origin.clone(),
    )
    .unwrap();
    let body = ExportDefaultBodyV1::try_new(
        vec![statement],
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::UnitLiteral,
            fixture.value_type.clone(),
            fixture.origin.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let templates = fixture.templates_with_body(body, CanonicalBooleanV1::False);

    assert_eq!(
        validate_envelopes(
            &templates,
            &fixture.callables(Effect::Ordinary),
            &fixture.sources(true),
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::NestedCallableAbi {
                index: 0,
                key: fixture.key,
                error: Box::new(DefaultNestedCallableAbiValidationError::Authority {
                    kind: DefaultNestedCallableKindV1::Lambda,
                    query: DefaultNestedCallableAuthorityQueryV1::Identity,
                    error: AuthorityError::NestedCallable,
                }),
            }
        )
    );
}

#[test]
fn routes_body_provider_type_failures_with_table_identity() {
    let fixture = Fixture::new();
    let invalid_type = SignatureTypeKey::Binder { depth: 0, index: 0 };
    let nested = DefaultStatementV1::try_new(
        DefaultStatementKindV1::Expr(Box::new(
            DefaultExpressionV1::try_new(
                DefaultExpressionKindV1::UnitLiteral,
                invalid_type,
                fixture.origin.clone(),
            )
            .unwrap(),
        )),
        fixture.origin.clone(),
    )
    .unwrap();
    let body = ExportDefaultBodyV1::try_new(
        vec![nested],
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::UnitLiteral,
            fixture.value_type.clone(),
            fixture.origin.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let templates = fixture.templates_with_body(body, CanonicalBooleanV1::False);

    assert_eq!(
        validate_envelopes(
            &templates,
            &fixture.callables(Effect::Ordinary),
            &fixture.sources(true),
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::Body {
                index: 0,
                key: fixture.key,
                error: Box::new(DefaultBodyProviderEnvelopeSemanticValidationError::Type {
                    site: DefaultBodyProviderTypeSiteV1::ExpressionResult,
                    definition_origin: Box::new(fixture.origin.clone()),
                    error: Box::new(SignatureTypeSemanticError::BinderScope(
                        SignatureBinderScopeError::DepthOutOfRange {
                            depth: 0,
                            available_depths: 0,
                        },
                    )),
                },),
            },
        )
    );
}

#[test]
fn body_validation_uses_the_caller_meter_and_path() {
    let fixture = Fixture::new();
    let templates = fixture.templates(CanonicalBooleanV1::False);
    let path = WirePath::root().field(11).index(3).field(5);
    let mut meter = BudgetMeter::new(DecodeLimits {
        semantic_recursion: 1,
        ..DecodeLimits::default()
    });
    let error = templates
        .validate_envelope_semantics(
            &fixture.callables(Effect::Ordinary),
            &fixture.sources(true),
            &mut fixture.authority(),
            &mut meter,
            &path,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        ExportDefaultTemplateSetEnvelopeSemanticValidationError::Body {
            index: 0,
            key,
            error,
        } if key == fixture.key
            && matches!(
                error.as_ref(),
                DefaultBodyProviderEnvelopeSemanticValidationError::Resource(error)
                    if error.kind()
                        == &WireErrorKind::LimitExceeded {
                            resource: ResourceKind::SemanticRecursion,
                            limit: 1,
                            observed: 2,
                        }
                        && error.path() == &path
            )
    ));
    assert_eq!(meter.usage().decoded_nodes, 1);
    assert_eq!(meter.usage().decoded_edges, 0);
    assert_eq!(meter.usage().validation_work_units, 1);
}

fn validate_envelopes(
    templates: &CanonicalExportDefaultTemplatesV1,
    callables: &CanonicalCallableInterfacesV1,
    sources: &CanonicalCallableSourceInterfacesV1,
    authority: &mut Authority,
) -> Result<(), ExportDefaultTemplateSetEnvelopeSemanticValidationError<AuthorityError>> {
    templates.validate_envelope_semantics(
        callables,
        sources,
        authority,
        &mut BudgetMeter::new(DecodeLimits::default()),
        &WirePath::root(),
    )
}

fn reference_set(
    fixture: &Fixture,
    target: ExportDefaultCallableTargetV1,
    call_domain: ExportDefaultCallDomainV1,
) -> ExportDefaultReferenceSetV1 {
    ExportDefaultReferenceSetV1::try_new(
        vec![ExportDefaultReferenceV1::new(
            target,
            fixture.origin.clone(),
            ExportDefaultAccessWitnessV1::new(fixture.owner, call_domain),
        )],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap()
}

struct Fixture {
    function: PersistentFunctionId,
    owner: CallableTemplateOrigin,
    key: ExportDefaultTemplateKeyV1,
    value_type_id: PersistentTypeId,
    value_type: SignatureTypeKey,
    definition_path: StructuralDefinitionPath,
    origin: ExportDefinitionSourceV1,
}

impl Fixture {
    fn new() -> Self {
        let nominal_key = SourceDeclarationKey::nominal(
            top_level_site(),
            identifier("Value"),
            SourceNominalKind::Struct,
            0,
        );
        let value_type_id = PersistentTypeId::from_source_declaration(&nominal_key).unwrap();
        let value_type = SignatureTypeKey::Nominal(value_type_id);
        let function_key = SourceDeclarationKey::function(
            top_level_site(),
            identifier("withDefault"),
            0,
            None,
            vec![value_type.clone()],
        );
        let function = PersistentFunctionId::from_source_declaration(&function_key).unwrap();
        let owner = CallableTemplateOrigin::Function(function);
        Self {
            function,
            owner,
            key: ExportDefaultTemplateKeyV1::new(owner, 0),
            value_type_id,
            value_type,
            definition_path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
                [],
            ),
            origin: origin(),
        }
    }

    fn callables(&self, effect: Effect) -> CanonicalCallableInterfacesV1 {
        let callable = CallableInterfaceRecordV1::try_new(
            self.owner,
            PublicDeclarationOwnerV1::TopLevel,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            None,
            CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
                identifier("value"),
                self.value_type.clone(),
            )])
            .unwrap(),
            self.value_type.clone(),
            effects(effect),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
        )
        .unwrap();
        CanonicalCallableInterfacesV1::try_new(vec![callable]).unwrap()
    }

    fn sources(&self, has_default: bool) -> CanonicalCallableSourceInterfacesV1 {
        let calling = if has_default {
            CallableParameterCallingV1::Default { template: self.key }
        } else {
            CallableParameterCallingV1::Required
        };
        let source = CallableSourceInterfaceV1::try_new(
            self.owner,
            CanonicalCallableSourceParametersV1::try_new(vec![CallableSourceParameterV1::new(
                identifier("value"),
                self.value_type.clone(),
                calling,
                self.origin.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
        CanonicalCallableSourceInterfacesV1::try_new(vec![source]).unwrap()
    }

    fn templates(&self, allows_suspend: CanonicalBooleanV1) -> CanonicalExportDefaultTemplatesV1 {
        let body = ExportDefaultBodyV1::try_new(
            Vec::new(),
            DefaultExpressionV1::try_new(
                DefaultExpressionKindV1::UnitLiteral,
                self.value_type.clone(),
                self.origin.clone(),
            )
            .unwrap(),
        )
        .unwrap();
        self.templates_with_body(body, allows_suspend)
    }

    fn templates_with_body(
        &self,
        body: ExportDefaultBodyV1,
        allows_suspend: CanonicalBooleanV1,
    ) -> CanonicalExportDefaultTemplatesV1 {
        self.templates_with_body_and_references(body, allows_suspend, self.value_type_references())
    }

    fn templates_with_references(
        &self,
        references: ExportDefaultReferenceSetV1,
    ) -> CanonicalExportDefaultTemplatesV1 {
        let body = ExportDefaultBodyV1::try_new(
            Vec::new(),
            DefaultExpressionV1::try_new(
                DefaultExpressionKindV1::UnitLiteral,
                self.value_type.clone(),
                self.origin.clone(),
            )
            .unwrap(),
        )
        .unwrap();
        self.templates_with_body_and_references(body, CanonicalBooleanV1::False, references)
    }

    fn templates_with_body_and_references(
        &self,
        body: ExportDefaultBodyV1,
        allows_suspend: CanonicalBooleanV1,
        references: ExportDefaultReferenceSetV1,
    ) -> CanonicalExportDefaultTemplatesV1 {
        self.templates_with_locals_body_and_references(
            CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
            body,
            allows_suspend,
            references,
        )
    }

    fn templates_with_locals_body_and_references(
        &self,
        locals: CanonicalTemplateLocalTableV1,
        body: ExportDefaultBodyV1,
        allows_suspend: CanonicalBooleanV1,
        references: ExportDefaultReferenceSetV1,
    ) -> CanonicalExportDefaultTemplatesV1 {
        let template = ExportDefaultTemplateV1::try_new(
            self.key,
            PersistentLexicalRootV1::Function(self.function),
            self.definition_path.clone(),
            locals,
            body,
            self.value_type.clone(),
            allows_suspend,
            CanonicalBinderUseListV1::try_new(Vec::new()).unwrap(),
            OptionalTemplateReceiverV1::Absent,
            CanonicalTemplateValueParametersV1::try_new(Vec::new()).unwrap(),
            references,
            self.origin.clone(),
        )
        .unwrap();
        CanonicalExportDefaultTemplatesV1::try_new(vec![template]).unwrap()
    }

    fn value_type_references(&self) -> ExportDefaultReferenceSetV1 {
        ExportDefaultReferenceSetV1::try_new(
            Vec::new(),
            Vec::new(),
            vec![ExportDefaultReferenceV1::new(
                self.value_type.clone(),
                self.origin.clone(),
                ExportDefaultAccessWitnessV1::new(
                    self.owner,
                    ExportDefaultCallDomainV1::DirectPublic,
                ),
            )],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    }

    fn authority(&self) -> Authority {
        Authority {
            owner: self.owner,
            value_type: self.value_type_id,
            provider_parameters: CanonicalSourceParameterShapesV1::try_new(vec![
                SourceParameterShapeV1::new(identifier("value"), self.value_type.clone()),
            ])
            .unwrap(),
            definition_path: self.definition_path.clone(),
            definition_source_validations: 0,
            template_origin_validations: 0,
            reject_template_origin: false,
            reject_reference_target: false,
        }
    }
}

struct Authority {
    owner: CallableTemplateOrigin,
    value_type: PersistentTypeId,
    provider_parameters: CanonicalSourceParameterShapesV1,
    definition_path: StructuralDefinitionPath,
    definition_source_validations: usize,
    template_origin_validations: usize,
    reject_template_origin: bool,
    reject_reference_target: bool,
}

impl NominalInterfaceShapeAuthority<AuthorityError> for Authority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, AuthorityError> {
        if declaration == self.value_type {
            Ok(PublicNominalShapeV1::new(PublicNominalKindV1::Struct, 0))
        } else {
            Err(AuthorityError::ConcreteNominal(declaration))
        }
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, AuthorityError> {
        Err(AuthorityError::GenericNominal(declaration))
    }
}

impl CallableInterfaceSemanticAuthority<AuthorityError> for Authority {
    fn callable_declaration_identity_shape(
        &mut self,
        declaration: CallableTemplateOrigin,
    ) -> Result<CallableDeclarationIdentityShapeV1, AuthorityError> {
        if declaration != self.owner {
            return Err(AuthorityError::Callable(declaration));
        }
        Ok(CallableDeclarationIdentityShapeV1::new(
            PublicDeclarationOwnerV1::TopLevel,
            0,
            0,
            None,
            vec![SignatureTypeKey::Nominal(self.value_type)],
        ))
    }
}

impl DefaultTemplateRootSemanticAuthority<AuthorityError> for Authority {
    fn default_template_provider_shape(
        &mut self,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
    ) -> Result<DefaultTemplateProviderShapeV1, AuthorityError> {
        if root.declaration() != self.owner || path != &self.definition_path {
            return Err(AuthorityError::Provider);
        }
        Ok(DefaultTemplateProviderShapeV1::try_new(0, 0).unwrap())
    }

    fn default_template_provider_parameter(
        &mut self,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
    ) -> Result<crate::DefaultTemplateProviderParameterV1<'_>, AuthorityError> {
        if root.declaration() != self.owner || path != &self.definition_path {
            return Err(AuthorityError::Provider);
        }
        crate::DefaultTemplateProviderParameterV1::try_new(&self.provider_parameters, 0)
            .map_err(|_| AuthorityError::Provider)
    }

    fn default_template_provider_receiver(
        &mut self,
        _root: crate::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
    ) -> Result<Option<SignatureTypeKey>, AuthorityError> {
        Ok(None)
    }

    fn validate_inherited_default_provider(
        &mut self,
        _key: ExportDefaultTemplateKeyV1,
        _root: PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _mapping: &crate::CanonicalBinderUseListV1,
    ) -> Result<(), AuthorityError> {
        Err(AuthorityError::UnexpectedInheritedProvider)
    }
}

impl ExportDefinitionSourceSemanticAuthority<AuthorityError> for Authority {
    fn current_cone(&self) -> ConeIdentity {
        ConeIdentity::SINGLE_FILE
    }

    fn validate_export_definition_source(
        &mut self,
        _source: &ExportDefinitionSourceV1,
    ) -> Result<(), AuthorityError> {
        self.definition_source_validations += 1;
        Ok(())
    }
}

impl DefaultTemplateOriginSemanticAuthority<AuthorityError> for Authority {
    fn validate_default_template_origin(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), AuthorityError> {
        if self.reject_template_origin
            || key.owner() != self.owner
            || root.declaration() != self.owner
            || path != &self.definition_path
        {
            return Err(AuthorityError::TemplateOrigin);
        }
        self.template_origin_validations += 1;
        Ok(())
    }

    fn validate_default_template_local_origin(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        _selector: &scoop_identity::LocalValueSelector,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), AuthorityError> {
        if key.owner() == self.owner
            && root.declaration() == self.owner
            && path == &self.definition_path
        {
            Ok(())
        } else {
            Err(AuthorityError::UnexpectedLocalOrigin)
        }
    }
}

impl DefaultLocalDataFlowSemanticAuthority<AuthorityError> for Authority {
    fn default_binding_struct_field_index(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        declaration: PersistentFieldId,
        _owner_type: &SignatureTypeKey,
    ) -> Result<u32, AuthorityError> {
        Err(AuthorityError::UnexpectedBindingStructField(declaration))
    }
}

impl DefaultOperationTypingSemanticAuthority<AuthorityError> for Authority {
    fn canonical_default_operation_type(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        role: DefaultOperationCoreTypeV1,
    ) -> Result<SignatureTypeKey, AuthorityError> {
        match role {
            DefaultOperationCoreTypeV1::Unit => Ok(SignatureTypeKey::Nominal(self.value_type)),
            DefaultOperationCoreTypeV1::Boolean => Ok(SignatureTypeKey::RawPointer(Box::new(
                SignatureTypeKey::Nominal(self.value_type),
            ))),
            _ => Err(AuthorityError::UnexpectedOperation),
        }
    }

    fn classify_default_core_application(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _value: &SignatureTypeKey,
    ) -> Result<Option<DefaultCoreApplicationV1>, AuthorityError> {
        Err(AuthorityError::UnexpectedOperation)
    }

    fn default_operation_entity_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _entity: DefaultOperationEntityV1<'_>,
    ) -> Result<DefaultOperationEntityShapeV1, AuthorityError> {
        Err(AuthorityError::UnexpectedOperation)
    }

    fn default_operation_type_relation(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _relation: DefaultOperationTypeRelationV1,
        _source: &SignatureTypeKey,
        _target: &SignatureTypeKey,
    ) -> Result<bool, AuthorityError> {
        Err(AuthorityError::UnexpectedOperation)
    }

    fn validate_default_operation_intrinsic(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _intrinsic: DefaultOperationIntrinsicV1<'_>,
    ) -> Result<(), AuthorityError> {
        Err(AuthorityError::UnexpectedOperation)
    }
}

impl DefaultNestedCallableSemanticAuthority<AuthorityError> for Authority {
    fn default_nested_callable_identity_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _identity: DefaultNestedCallableIdentityV1,
        _site: crate::DefaultNestedCallableSiteV1,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, AuthorityError> {
        Err(AuthorityError::NestedCallable)
    }

    fn default_nested_callable_abi_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _identity: DefaultNestedCallableIdentityV1,
        _site: crate::DefaultNestedCallableSiteV1,
        _body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
    ) -> Result<DefaultNestedCallableAbiShapeV1, AuthorityError> {
        Err(AuthorityError::NestedCallable)
    }
}

impl DefaultReferenceSemanticAuthority<AuthorityError> for Authority {
    fn validate_default_callable_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &ExportDefaultCallableTargetV1,
    ) -> Result<(), AuthorityError> {
        if self.reject_reference_target {
            Err(AuthorityError::ReferenceTarget)
        } else {
            Ok(())
        }
    }

    fn validate_default_constructor_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &DefaultConstructorRefV1,
    ) -> Result<(), AuthorityError> {
        Err(AuthorityError::UnexpectedReferenceTarget(
            ExportDefaultReferenceKindV1::Constructor,
        ))
    }

    fn validate_default_type_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        target: &SignatureTypeKey,
    ) -> Result<(), AuthorityError> {
        if target == &SignatureTypeKey::Nominal(self.value_type) {
            Ok(())
        } else {
            Err(AuthorityError::UnexpectedReferenceTarget(
                ExportDefaultReferenceKindV1::Type,
            ))
        }
    }

    fn validate_default_global_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: PersistentPropertyId,
    ) -> Result<(), AuthorityError> {
        Err(AuthorityError::UnexpectedReferenceTarget(
            ExportDefaultReferenceKindV1::Global,
        ))
    }

    fn validate_default_singleton_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: PersistentObjectValueId,
    ) -> Result<(), AuthorityError> {
        Err(AuthorityError::UnexpectedReferenceTarget(
            ExportDefaultReferenceKindV1::Singleton,
        ))
    }

    fn validate_default_field_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &DefaultFieldRefV1,
    ) -> Result<(), AuthorityError> {
        Err(AuthorityError::UnexpectedReferenceTarget(
            ExportDefaultReferenceKindV1::Field,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthorityError {
    Callable(CallableTemplateOrigin),
    ConcreteNominal(PersistentTypeId),
    GenericNominal(PersistentGenericTypeId),
    Provider,
    UnexpectedInheritedProvider,
    TemplateOrigin,
    UnexpectedLocalOrigin,
    UnexpectedBindingStructField(PersistentFieldId),
    UnexpectedOperation,
    NestedCallable,
    ReferenceTarget,
    UnexpectedReferenceTarget(ExportDefaultReferenceKindV1),
}

impl std::fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing table-envelope authority: {self:?}")
    }
}

impl std::error::Error for AuthorityError {}

fn effects(execution: Effect) -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        execution,
        CallableSafetyV1::Safe,
        GcEffect::Managed,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}

fn top_level_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn origin() -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::single_file();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(1, 2).unwrap(), &context).unwrap(),
    )
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
