use std::path::Path;

use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::*;

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
            CurrentConeRequestV1::ManifestRoot {
                root: path("edited-library"),
            },
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
    let decoded = decode_canonical::<DecodedScoopcRequestEnvelopeV1>(
        &encode(&request).unwrap(),
        DecodeLimits::M23_DEFAULT,
    )
    .unwrap()
    .validate()
    .unwrap();
    assert_eq!(decoded.build().direct_slibs(), &[path("helper.slib")]);
    assert_eq!(decoded.build().support_slibs(), &[path("support.slib")]);
    assert_eq!(decoded, request);
}

#[test]
fn request_constructor_closes_single_file_protocol_combinations() {
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
        StageDumpPolicyV1::Stage(StageDumpKindV1::Hir),
    )
    .unwrap_err();
    assert_eq!(
        error,
        ProtocolValidationError::InvalidCurrentCoreCombination
    );

    let error = ScoopcBuildRequestV1::new(
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
    .unwrap_err();
    assert_eq!(error, ProtocolValidationError::SingleFileHasDependencies);

    let input = path("dependency.slib");
    let error = ScoopcBuildRequestV1::new(
        CurrentConeRequestV1::SingleFile {
            source: path("main.scoop"),
        },
        vec![input; MAX_INPUT_ARTIFACTS + 1],
        Vec::new(),
        TrustedCoreRequestV1::ArtifactSlot {
            artifact: path("core.slib"),
        },
        target(),
        path("main.slib"),
        DiagnosticOutputPolicyV1::Human,
        StageDumpPolicyV1::None,
    )
    .unwrap_err();
    assert_eq!(
        error,
        ProtocolValidationError::TooManyInputs {
            role: "direct",
            actual: MAX_INPUT_ARTIFACTS + 1,
        }
    );
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
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 9 });

    let error = decode_canonical::<DecodedCurrentConeRequestV1>(
        &encode(&RawCurrentCone { fields: 1, tag: 1 }).unwrap(),
        DecodeLimits::default(),
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
fn removed_pathless_core_request_tag_is_rejected() {
    let error = decode_canonical::<DecodedCurrentConeRequestV1>(
        &[0xa1, 0x00, 0x03],
        DecodeLimits::M23_DEFAULT,
    )
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}

#[test]
fn core_manifest_dependencies_use_the_common_input_limits() {
    for role in ["direct", "support"] {
        let inputs = vec![path("dependency.slib"); MAX_INPUT_ARTIFACTS + 1];
        let (direct, support) = if role == "direct" {
            (inputs, Vec::new())
        } else {
            (Vec::new(), inputs)
        };
        let error = ScoopcBuildRequestV1::new(
            CurrentConeRequestV1::ManifestRoot { root: path("core") },
            direct,
            support,
            TrustedCoreRequestV1::Bootstrap,
            target(),
            path("core.slib"),
            DiagnosticOutputPolicyV1::Structured,
            StageDumpPolicyV1::None,
        )
        .unwrap_err();
        assert_eq!(
            error,
            ProtocolValidationError::TooManyInputs {
                role,
                actual: MAX_INPUT_ARTIFACTS + 1
            }
        );
    }
}
