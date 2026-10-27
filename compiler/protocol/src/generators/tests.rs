use super::*;

#[test]
fn used_program_round_trips_host_path_version_and_timestamp() {
    for modified in [
        UNIX_EPOCH + Duration::new(7, 123),
        UNIX_EPOCH - Duration::new(2, 1),
    ] {
        let path = std::env::temp_dir().join("generator with spaces");
        let registration = GeneratorRegistrationV1::new(
            "example".into(),
            HostPathCarrier::from_path(&path).unwrap(),
        )
        .unwrap();
        let input = GeneratorToolInputV1::new(registration, 1, 42, modified);
        let bytes = scoop_wire::encode(&input).unwrap();
        assert_eq!(
            scoop_wire::decode_canonical::<GeneratorToolInputV1>(&bytes).unwrap(),
            input
        );
    }
}

#[test]
fn generator_registration_requires_an_actual_name() {
    let path = HostPathCarrier::from_path(&std::env::temp_dir().join("generator")).unwrap();
    assert_eq!(
        GeneratorRegistrationV1::new(" ".into(), path),
        Err(ProtocolValidationError::InvalidGeneratorRegistration)
    );
}

#[test]
fn build_transport_preserves_registered_and_used_programs() {
    use crate::*;
    let root = std::env::temp_dir();
    let host = |name| HostPathCarrier::from_path(&root.join(name)).unwrap();
    let registration = GeneratorRegistrationV1::new("custom".into(), host("generator")).unwrap();
    let build = ScoopcBuildRequestV1::new(
        crate::test_support::manifest("library"),
        vec![],
        vec![],
        TrustedCoreRequestV1::Bootstrap,
        TargetSelectionRequestV1::new("aarch64-apple-darwin".into()).unwrap(),
        host("output.slib"),
        DiagnosticOutputPolicyV1::Structured,
        StageDumpPolicyV1::None,
    )
    .unwrap();
    assert!(
        build
            .clone()
            .with_generators(vec![registration.clone(), registration.clone()])
            .is_err()
    );
    let build = build.with_generators(vec![registration.clone()]).unwrap();
    let id = RequestCorrelationId::from_array([9; 16]);
    let request = ScoopcRequestEnvelopeV1::new(id, build);
    assert_eq!(
        decode_request_frame(&encode_request_frame(&request).unwrap()).unwrap(),
        request
    );
    let input = GeneratorToolInputV1::new(registration, 1, 15, UNIX_EPOCH + Duration::from_secs(3));
    let result = ScoopcSuccessV1::new(crate::test_summary(), vec![], vec![])
        .unwrap()
        .with_used_generators(vec![input]);
    let response = ScoopcResponseEnvelopeV1::success(id, result);
    assert_eq!(
        decode_response_frame(&encode_response_frame(&response).unwrap()).unwrap(),
        response
    );
}
