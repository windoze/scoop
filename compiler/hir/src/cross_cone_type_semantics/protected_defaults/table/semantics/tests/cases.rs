use super::*;

fn template_error(case: Case) -> ProtectedDefaultSemanticError<&'static str> {
    match run(case).unwrap_err() {
        ProtectedDefaultTableSemanticError::Template { index, key, error } => {
            assert_eq!(index, 0);
            assert_eq!(key.parameter_position(), 1);
            *error
        }
        other => panic!("expected template failure for {case:?}: {other:?}"),
    }
}
#[test]
fn complete_default_tables_join_real_sources_receiver_prefix_flow_and_nested_abi() {
    for case in [Case::Valid, Case::Flow, Case::Nested] {
        let result = run(case).unwrap();
        assert!(!result.metadata);
        assert_eq!(result.provider_calls, 1);
        assert!(result.origin_calls >= 3);
        assert!(result.metadata_calls >= 2);
        assert!(result.expression_calls > 0);
        assert_eq!(
            result.concrete_calls,
            result.metadata_calls + result.expression_calls
        );

        if case == Case::Flow {
            assert!(result.body_calls > 0);
        }
        assert_eq!(
            result.nested_calls,
            if case == Case::Nested { 2 } else { 0 }
        );
    }
}
#[test]
fn full_generic_metadata_proof_retains_source_checks_without_param_free_conversion() {
    let result = run(Case::Generic).unwrap();
    assert!(result.metadata);
    assert_eq!(result.provider_calls, 1);
    assert_eq!(result.origin_calls, 3);
    assert_eq!(result.concrete_calls, 0);
    assert_eq!(result.metadata_calls, 2);
    assert_eq!(result.expression_calls, 1);
    let ProtectedDefaultSemanticError::References(error) = template_error(Case::GenericReject)
    else {
        panic!("expected generic source replay failure");
    };
    assert!(matches!(
        *error,
        ProtectedDefaultReferenceSemanticError::Body(ProtectedDefaultBodyClosureError::Source(
            ProtectedDefaultReferenceAccessError::Source("independent reference access denied")
        ))
    ));
}
#[test]
fn complete_default_proof_rejects_contract_body_envelope_and_origin_failures() {
    let ProtectedDefaultSemanticError::Contract(error) = template_error(Case::Contract) else {
        panic!("expected contract failure");
    };
    assert!(matches!(
        *error,
        ProtectedDefaultTemplateContractSemanticError::SuspendPermission
    ));
    let ProtectedDefaultSemanticError::BodyEnvelope(error) = template_error(Case::BodyEnvelope)
    else {
        panic!("expected provider envelope failure");
    };
    let DefaultBodyProviderEnvelopeSemanticValidationError::Type {
        site: DefaultBodyProviderTypeSiteV1::ExpressionResult,
        error,
        ..
    } = *error
    else {
        panic!("expected expression signature scope failure");
    };
    assert!(matches!(
        *error,
        SignatureTypeSemanticError::BinderScope(SignatureBinderScopeError::DepthOutOfRange {
            depth: 0,
            available_depths: 0
        })
    ));
    let ProtectedDefaultSemanticError::Origin(error) = template_error(Case::Origin) else {
        panic!("expected definition subject failure");
    };
    assert!(matches!(
        *error,
        ProtectedDefaultTemplateOriginSemanticError::DefinitionRelation(
            "wrong default source subject"
        )
    ));
}
#[test]
fn complete_default_proof_rejects_before_definition_wrong_operation_and_nested_capture_abi() {
    let ProtectedDefaultSemanticError::LocalDataFlow(error) =
        template_error(Case::BeforeDefinition)
    else {
        panic!("expected flow failure");
    };
    assert!(matches!(
        *error,
        ExportDefaultLocalDataFlowValidationError::Local {
            error: DefaultLocalDataFlowLocalError::UseBeforeDefinition,
            ..
        }
    ));
    let ProtectedDefaultSemanticError::OperationTyping(error) = template_error(Case::OperationType)
    else {
        panic!("expected operation type failure");
    };
    let ExportDefaultBodyOperationTypingValidationError::Expression(error) = *error else {
        panic!("expected invalid unit expression");
    };
    assert!(matches!(
        *error,
        ExportDefaultOperationTypingValidationError::Type { .. }
    ));
    let ProtectedDefaultSemanticError::NestedCallableAbi(error) = template_error(Case::NestedAbi)
    else {
        panic!("expected nested ABI failure");
    };
    assert!(matches!(
        *error,
        DefaultNestedCallableAbiValidationError::CaptureArity { .. }
    ));
}
#[test]
fn complete_default_proof_requires_exact_metadata_uses_actual_source_and_domains() {
    for case in [
        Case::MissingMetadata,
        Case::WrongUse,
        Case::ReferenceDomain,
        Case::ReferenceSource,
    ] {
        let ProtectedDefaultSemanticError::References(error) = template_error(case) else {
            panic!("expected complete reference replay failure");
        };
        let ProtectedDefaultReferenceSemanticError::Body(error) = *error else {
            panic!("expected body closure failure");
        };
        match case {
            Case::MissingMetadata => assert!(matches!(
                error,
                ProtectedDefaultBodyClosureError::Missing {
                    kind: ProtectedDefaultReferenceKindV1::Type,
                    site: ExportDefaultReferenceOccurrenceSiteV1::TemplateLocalType { .. },
                    ..
                }
            )),
            Case::WrongUse => assert!(matches!(
                error,
                ProtectedDefaultBodyClosureError::MissingUse {
                    kind: ProtectedDefaultReferenceKindV1::Type,
                    ..
                }
            )),
            Case::ReferenceDomain => assert!(matches!(
                error,
                ProtectedDefaultBodyClosureError::Source(
                    ProtectedDefaultReferenceAccessError::Coverage(
                        ProtectedDefaultDomainCoverageError::DomainMismatch
                    )
                )
            )),
            Case::ReferenceSource => assert!(matches!(
                error,
                ProtectedDefaultBodyClosureError::Source(
                    ProtectedDefaultReferenceAccessError::Source(
                        "independent reference access denied"
                    )
                )
            )),
            _ => unreachable!(),
        }
    }
}
#[test]
fn checked_source_protocol_cannot_authorize_missing_or_extra_default_records() {
    for case in [Case::MissingDefault, Case::ExtraDefault] {
        assert!(matches!(
            run(case),
            Err(ProtectedDefaultTableSemanticError::SourceClosure(
                ProtectedSourceIndexError::Build(ProtectedSourceBuildError::DefaultClosure)
            ))
        ));
    }
}
