use std::path::Path;

use scoop_wire::{WireErrorKind, decode_canonical, encode};

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
    let decoded = decode_canonical::<DecodedScoopcRequestEnvelopeV1>(&encode(&request).unwrap())
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
        &encode(&RawCurrentCone { fields: 1, tag: 1 }).unwrap(),
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
    let error = decode_canonical::<DecodedCurrentConeRequestV1>(&[0xa1, 0x00, 0x03]).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
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
        let request = TargetSelectionRequestV1::new("x86_64-unknown-linux-musl".into())
            .unwrap()
            .with_c_toolchain(compiler, sysroot);
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
            expected: 3,
            actual: 1
        }
    ));
    let multiple_drivers = [0xa3, 0x01, 0x61, b'x', 0x02, 0x82, 0, 0, 0x03, 0x80];
    let error = decode_canonical::<DecodedTargetSelectionRequestV1>(&multiple_drivers).unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2
        }
    ));
}
