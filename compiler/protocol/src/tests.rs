//! Protocol round-trip and rejection tests.

use crate::{
    BuildOutcome, BuildRequest, CompilerIdentity, DiagnosticPhase, Hello, Message,
    PROTOCOL_VERSION, ProtocolDiagnostic, read_frame, write_frame,
};

use scoop_identity::{ConeCoordinate, Digest256};

fn sample_request() -> BuildRequest {
    BuildRequest {
        cone_root: "/build/app".to_owned(),
        core_slib: "/sysroot/core/cone.slib".to_owned(),
        direct_slibs: vec!["/cache/org.foo:bar:1.2.3.slib".to_owned()],
        support_slibs: vec![
            "/cache/org.other:log:3.1.0.slib".to_owned(),
            "/cache/org.deep:x:0.0.1.slib".to_owned(),
        ],
        out_slib: "/tmp/out/cone.slib".to_owned(),
    }
}

#[test]
fn hello_round_trip() {
    let message = Message::Hello(Hello {
        identity: CompilerIdentity::current(),
        toolchain_stamp: Some(Digest256::from_bytes([7; 32])),
    });
    let decoded = Message::decode(&message.encode()).expect("round trip");
    assert_eq!(decoded, message);
}

#[test]
fn hello_without_stamp_round_trip() {
    let message = Message::Hello(Hello {
        identity: CompilerIdentity::current(),
        toolchain_stamp: None,
    });
    let decoded = Message::decode(&message.encode()).expect("round trip");
    assert_eq!(decoded, message);
}

#[test]
fn build_request_round_trip() {
    let message = Message::BuildRequest(sample_request());
    let decoded = Message::decode(&message.encode()).expect("round trip");
    assert_eq!(decoded, message);
}

#[test]
fn build_outcome_round_trip_both_variants() {
    let ok = Message::BuildOutcome(BuildOutcome::Success {
        coordinate: ConeCoordinate::new("dev.example", "app", "0.1.0").unwrap(),
        warnings: vec![ProtocolDiagnostic {
            severity: crate::Severity::Warning,
            phase: DiagnosticPhase::Source,
            message: "shadowed declaration".to_owned(),
            source: Some(crate::SourceLocation {
                path: "src/main.scoop".to_owned(),
                span_start: 4,
                span_end: 9,
            }),
        }],
    });
    let failure = Message::BuildOutcome(BuildOutcome::Failure {
        diagnostics: vec![
            ProtocolDiagnostic {
                severity: crate::Severity::Error,
                phase: DiagnosticPhase::Manifest,
                message: "unknown cone kind".to_owned(),
                source: Some(crate::SourceLocation {
                    path: "Cone.toml".to_owned(),
                    span_start: 12,
                    span_end: 30,
                }),
            },
            ProtocolDiagnostic {
                severity: crate::Severity::Error,
                phase: DiagnosticPhase::SlibReader,
                message: "payload hash mismatch".to_owned(),
                source: None,
            },
        ],
    });
    assert_eq!(Message::decode(&ok.encode()).expect("ok round trip"), ok);
    assert_eq!(
        Message::decode(&failure.encode()).expect("failure round trip"),
        failure
    );
}

#[test]
fn frame_round_trip() {
    let message = Message::BuildRequest(sample_request());
    let mut buffer = Vec::new();
    write_frame(&mut buffer, &message).expect("write");
    assert_eq!(read_frame(&mut buffer.as_slice()).expect("read"), message);
    // Trailing bytes after one frame are rejected by decode's finish.
    let mut doubled = buffer.clone();
    doubled.extend_from_slice(&buffer);
    assert!(read_frame(&mut doubled.as_slice()).is_ok()); // first frame only
}

#[test]
fn truncated_frame_is_rejected() {
    let message = Message::BuildRequest(sample_request());
    let mut buffer = Vec::new();
    write_frame(&mut buffer, &message).expect("write");
    let cut = buffer[..buffer.len() - 3].to_vec();
    assert!(read_frame(&mut cut.as_slice()).is_err());
}

#[test]
fn unknown_message_tag_is_rejected() {
    // Hand-build `{0: 9, 1: {}}`.
    let mut writer = scoop_identity::CborWriter::new();
    writer.map(2);
    writer.field(0).unsigned(9);
    writer.field(1).map(0);
    let data = writer.into_bytes();
    assert!(matches!(
        Message::decode(&data).unwrap_err(),
        crate::ProtocolError::UnknownMessageTag(9)
    ));
}

#[test]
fn tampered_payload_is_rejected() {
    let message = Message::BuildOutcome(BuildOutcome::Failure {
        diagnostics: Vec::new(),
    });
    let mut bytes = message.encode();
    // Flip a byte in the middle.
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0x01;
    assert!(Message::decode(&bytes).is_err());
}

#[test]
fn current_identity_is_protocol_version_one() {
    let identity = CompilerIdentity::current();
    assert_eq!(identity.protocol_version, PROTOCOL_VERSION);
    assert!(identity.compatible(&CompilerIdentity::current()));

    let mut other = CompilerIdentity::current();
    other.hir_wire_schema += 1;
    assert!(!identity.compatible(&other));
    let mut other = CompilerIdentity::current();
    other.compiler_version = "0.0.0-nightly".to_owned();
    // Compiler version is diagnostic only.
    assert!(identity.compatible(&other));
}
