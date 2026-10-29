use std::path::Path;

use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

mod check;
mod documentation;

#[test]
fn expanded_input_retains_configuration_and_rejects_invalid_diagnostic_spans() {
    let CurrentConeRequestV1::ExpandedCone { mut config } =
        crate::test_support::manifest("library")
    else {
        panic!()
    };
    let encoded = serde_json::to_vec(&config).unwrap();
    let decoded: crate::package::ExpandedConeInputV1 = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded, *config);
    config.manifest_text.clear();
    assert!(matches!(
        ScoopcBuildRequestV1::new(
            CurrentConeRequestV1::ExpandedCone { config }, Vec::new(), Vec::new(),
            TrustedCoreRequestV1::Bootstrap, target(), ScoopcOutputRequestV1::Check,
            DiagnosticOutputPolicyV1::Structured, StageDumpPolicyV1::None,
        ),
        Err(ProtocolValidationError::InvalidExpandedCone(message)) if message.contains("diagnostic span")
    ));
}

fn path(value: &str) -> HostPathCarrier {
    HostPathCarrier::from_path(Path::new(value)).unwrap()
}

fn target() -> TargetSelectionRequestV1 {
    TargetSelectionRequestV1::new("aarch64-apple-darwin".to_owned()).unwrap()
}

#[test]
fn core_manifest_request_round_trips_without_a_default_source_slot() {
    let request = ScoopcRequestEnvelopeV1::new(
        RequestCorrelationId::from_array([72; 16]),
        ScoopcBuildRequestV1::new(
            crate::test_support::manifest("edited-library"),
            vec![path("helper.slib")],
            vec![path("support.slib")],
            TrustedCoreRequestV1::Bootstrap,
            target(),
            path("custom-output/core.slib"),
            DiagnosticOutputPolicyV1::Structured,
            StageDumpPolicyV1::None,
        )
        .unwrap(),
    );
    let decoded = decode_canonical::<DecodedScoopcRequestEnvelopeV1>(&encode(&request).unwrap())
        .unwrap()
        .validate()
        .unwrap();
    assert_eq!(decoded.build().direct_slibs(), &[path("helper.slib")]);
    assert_eq!(decoded.build().support_slibs(), &[path("support.slib")]);
    assert_eq!(decoded, request);
    let release = ScoopcRequestEnvelopeV1::new(
        request.request_id(),
        request
            .build()
            .clone()
            .with_optimization(OptimizationMode::Release),
    );
    let bytes = encode(&release).unwrap();
    assert_ne!(bytes, encode(&request).unwrap());
    let decoded = decode_canonical::<DecodedScoopcRequestEnvelopeV1>(&bytes)
        .unwrap()
        .validate()
        .unwrap();
    assert_eq!(decoded.build().optimization(), OptimizationMode::Release);
    assert_eq!(decoded.build().target(), request.build().target());
    let mut unknown = bytes;
    let optimization = unknown.len() - 11;
    assert_eq!(unknown[optimization], 2);
    unknown[optimization] = 3;
    assert!(decode_canonical::<DecodedScoopcRequestEnvelopeV1>(&unknown).is_err());

    let native = request
        .build()
        .clone()
        .with_native_inputs(vec![path("unit.i")]);
    let native = ScoopcRequestEnvelopeV1::new(request.request_id(), native);
    let decoded = decode_canonical::<DecodedScoopcRequestEnvelopeV1>(&encode(&native).unwrap())
        .unwrap()
        .validate()
        .unwrap();
    assert_eq!(decoded.build().native_inputs(), &[path("unit.i")]);
    let test = ScoopcRequestEnvelopeV1::new(
        request.request_id(),
        request
            .build()
            .clone()
            .with_purpose(crate::CompilationPurpose::Test)
            .unwrap(),
    );
    let bytes = encode(&test).unwrap();
    assert_ne!(bytes, encode(&request).unwrap());
    assert_eq!(
        decode_canonical::<DecodedScoopcRequestEnvelopeV1>(&bytes)
            .unwrap()
            .validate()
            .unwrap(),
        test
    );
    let mut unknown_purpose = bytes;
    let purpose = unknown_purpose.len() - 7;
    assert_eq!(unknown_purpose[purpose], 2);
    unknown_purpose[purpose] = 3;
    assert!(decode_canonical::<DecodedScoopcRequestEnvelopeV1>(&unknown_purpose).is_err());
    let mut missing = encode(request.build()).unwrap();
    let purpose = missing.len() - 6;
    assert_eq!(&missing[purpose..], &[11, 1, 12, 0x80, 13, 0]);
    missing.drain(purpose..purpose + 2);
    missing[0] = 0xac;
    assert!(decode_canonical::<super::decode::DecodedScoopcBuildRequestV1>(&missing).is_err());
}

#[test]
fn request_constructor_closes_single_file_protocol_combinations() {
    let single_file = ScoopcBuildRequestV1::new(
        CurrentConeRequestV1::SingleFile {
            source: path("main.scoop"),
        },
        Vec::new(),
        Vec::new(),
        TrustedCoreRequestV1::ArtifactSlot {
            artifact: path("core.slib"),
        },
        target(),
        path("main.slib"),
        DiagnosticOutputPolicyV1::Human,
        StageDumpPolicyV1::None,
    )
    .unwrap();
    assert_eq!(single_file.purpose(), crate::CompilationPurpose::Production);
    assert_eq!(
        single_file
            .with_purpose(crate::CompilationPurpose::Test)
            .unwrap_err(),
        ProtocolValidationError::TestRequiresManifest
    );
    let error = ScoopcBuildRequestV1::new(
        CurrentConeRequestV1::SingleFile {
            source: path("main.scoop"),
        },
        Vec::new(),
        Vec::new(),
        TrustedCoreRequestV1::Bootstrap,
        target(),
        path("main.slib"),
        DiagnosticOutputPolicyV1::Human,
        StageDumpPolicyV1::Files {
            stages: crate::StageDumpSet::one(crate::StageDumpKindV1::Hir),
            directory: path("dump"),
        },
    )
    .unwrap_err();
    assert_eq!(
        error,
        ProtocolValidationError::InvalidCurrentCoreCombination
    );

    let request = ScoopcBuildRequestV1::new(
        CurrentConeRequestV1::SingleFile {
            source: path("main.scoop"),
        },
        vec![path("dependency.slib")],
        Vec::new(),
        TrustedCoreRequestV1::ArtifactSlot {
            artifact: path("core.slib"),
        },
        target(),
        path("main.slib"),
        DiagnosticOutputPolicyV1::Human,
        StageDumpPolicyV1::None,
    )
    .unwrap();
    assert_eq!(request.direct_slibs(), &[path("dependency.slib")]);
}

struct RawCurrentCone {
    fields: u64,
    tag: u64,
}

impl WireEncode for RawCurrentCone {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(self.fields)?;
        encoder.field(0)?;
        encoder.unsigned(self.tag)
    }
}

#[test]
fn current_cone_reader_distinguishes_unknown_tag_and_wrong_sum_length() {
    let error = decode_canonical::<DecodedCurrentConeRequestV1>(
        &encode(&RawCurrentCone { fields: 1, tag: 9 }).unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 9 });

    let error = decode_canonical::<DecodedCurrentConeRequestV1>(
        &encode(&RawCurrentCone { fields: 1, tag: 3 }).unwrap(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );
}

#[test]
fn removed_manifest_root_request_tag_is_rejected() {
    let error = decode_canonical::<DecodedCurrentConeRequestV1>(&[0xa1, 0x00, 0x01]).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 1 });
}

#[test]
fn target_request_retains_native_tool_paths_and_rejects_the_old_shape() {
    for (compiler, sysroot) in [
        (None, None),
        (Some(path("/opt/musl/bin/musl-gcc")), None),
        (
            Some(path("/opt/gcc/bin/gcc")),
            Some(path("/opt/target sysroot")),
        ),
    ] {
        let request = TargetSelectionRequestV1::new("x86_64-unknown-linux-gnu".into())
            .unwrap()
            .with_native_toolchain(compiler, sysroot, Some(path("/tools/compiler cxx")));
        let decoded =
            decode_canonical::<DecodedTargetSelectionRequestV1>(&encode(&request).unwrap())
                .unwrap()
                .validate()
                .unwrap();
        assert_eq!(decoded, request);
    }
    let old_shape = [0xa1, 0x01, 0x61, b'x'];
    let error = decode_canonical::<DecodedTargetSelectionRequestV1>(&old_shape).unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1
        }
    ));
    let multiple_drivers = [
        0xa2, 0x01, 0x61, b'x', 0x02, 0x84, 0, 0x82, 0, 0, 0x80, 0x80,
    ];
    let error = decode_canonical::<DecodedTargetSelectionRequestV1>(&multiple_drivers).unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2
        }
    ));
}

#[test]
fn target_request_transports_completed_tool_discovery() {
    let record = b"resolved tool invocation".to_vec();
    let request = target().with_resolved_toolchain(record.clone());
    let decoded = decode_canonical::<DecodedTargetSelectionRequestV1>(&encode(&request).unwrap())
        .unwrap()
        .validate()
        .unwrap();
    assert_eq!(decoded, request);
    assert_eq!(decoded.resolved_toolchain(), Some(record.as_slice()));
    assert!(decoded.compiler().is_none());
    assert!(decoded.native_sysroot().is_none());
    assert!(decoded.cxx_compiler().is_none());
}
