use super::*;

#[test]
fn check_success_preserves_warnings_without_artifact_or_documentation() {
    let warning = diagnostic(DiagnosticSeverityV1::Warning, "package is ignored".into());
    let success =
        ScoopcSuccessV1::new(ScoopcProductV1::Check, vec![warning.clone()], Vec::new()).unwrap();
    let response =
        ScoopcResponseEnvelopeV1::success(RequestCorrelationId::from_array([23; 16]), success);
    let decoded =
        crate::decode_response_frame(&crate::encode_response_frame(&response).unwrap()).unwrap();
    assert_eq!(decoded, response);
    let ScoopcResponseEnvelopeV1::Success { result, .. } = decoded else {
        panic!("expected success")
    };
    assert!(matches!(result.product(), ScoopcProductV1::Check));
    assert!(result.summary().is_none());
    assert!(result.documentation().is_none());
    assert_eq!(result.warnings(), &[warning]);
    assert_eq!(
        ScoopcSuccessV1::new(
            ScoopcProductV1::Check,
            Vec::new(),
            vec![EmittedDumpDescriptorV1::new(
                crate::StageDumpKindV1::Hir,
                EmittedDumpDestinationV1::File(
                    HostPathCarrier::from_path(std::path::Path::new("hir.txt")).unwrap()
                )
            )]
        )
        .unwrap_err(),
        ProtocolValidationError::InvalidDumpDescriptors
    );
}
