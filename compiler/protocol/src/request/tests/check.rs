use super::*;

fn request(current: CurrentConeRequestV1) -> ScoopcBuildRequestV1 {
    ScoopcBuildRequestV1::new(
        current,
        vec![path("dependency.slib")],
        Vec::new(),
        TrustedCoreRequestV1::ArtifactSlot {
            artifact: path("core.slib"),
        },
        target(),
        ScoopcOutputRequestV1::Check,
        DiagnosticOutputPolicyV1::Structured,
        StageDumpPolicyV1::None,
    )
    .unwrap()
}

#[test]
fn check_requests_round_trip_without_output_paths() {
    for current in [
        crate::test_support::manifest("library"),
        CurrentConeRequestV1::SingleFile {
            source: path("main.scoop"),
        },
    ] {
        let request = ScoopcRequestEnvelopeV1::new(
            RequestCorrelationId::from_array([23; 16]),
            request(current),
        );
        let decoded =
            crate::decode_request_frame(&crate::encode_request_frame(&request).unwrap()).unwrap();
        assert_eq!(decoded, request);
        assert!(decoded.build().output().artifact().is_none());
        assert!(decoded.build().output().documentation().is_none());
    }
}

#[test]
fn checks_support_test_purpose_but_reject_machine_inputs_and_observations() {
    let request = request(crate::test_support::manifest("library"));
    let tests = request
        .clone()
        .with_purpose(crate::CompilationPurpose::Test)
        .unwrap();
    let envelope = ScoopcRequestEnvelopeV1::new(RequestCorrelationId::from_array([23; 16]), tests);
    assert_eq!(
        crate::decode_request_frame(&crate::encode_request_frame(&envelope).unwrap()).unwrap(),
        envelope
    );
    assert_eq!(
        envelope.observe(crate::StageDumpSet::all()).unwrap_err(),
        ProtocolValidationError::InvalidCheckRequest
    );
    let invalid = ScoopcRequestEnvelopeV1::new(
        RequestCorrelationId::from_array([23; 16]),
        request.with_native_inputs(vec![path("native.o")]),
    );
    assert!(crate::decode_request_frame(&crate::encode_request_frame(&invalid).unwrap()).is_err());
    assert_eq!(
        ScoopcBuildRequestV1::new(
            crate::test_support::manifest("library"),
            Vec::new(),
            Vec::new(),
            TrustedCoreRequestV1::Bootstrap,
            target(),
            ScoopcOutputRequestV1::Check,
            DiagnosticOutputPolicyV1::Structured,
            StageDumpPolicyV1::Files {
                stages: crate::StageDumpSet::all(),
                directory: path("dumps")
            },
        )
        .unwrap_err(),
        ProtocolValidationError::InvalidCheckRequest
    );
}
