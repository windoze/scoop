use super::*;
use crate::{CompilationPurpose, StageDumpKindV1, StageDumpSet};

fn build(
    output: ScoopcOutputRequestV1,
    current: CurrentConeRequestV1,
    emit: StageDumpPolicyV1,
) -> Result<ScoopcBuildRequestV1, ProtocolValidationError> {
    ScoopcBuildRequestV1::new(
        current,
        Vec::new(),
        Vec::new(),
        TrustedCoreRequestV1::ArtifactSlot {
            artifact: path("core.slib"),
        },
        target(),
        output,
        DiagnosticOutputPolicyV1::Structured,
        emit,
    )
}

fn manifest() -> CurrentConeRequestV1 {
    CurrentConeRequestV1::ManifestRoot { root: path("lib") }
}

fn documentation() -> ScoopcOutputRequestV1 {
    ScoopcOutputRequestV1::Documentation(path("lib.json"))
}

fn dumps(stage: StageDumpKindV1) -> StageDumpPolicyV1 {
    StageDumpPolicyV1::Files {
        stages: StageDumpSet::one(stage),
        directory: path("dumps"),
    }
}

#[test]
fn documentation_output_intents_round_trip_without_an_artificial_slib() {
    for output in [
        ScoopcOutputRequestV1::Artifact(path("lib.slib")),
        documentation(),
        ScoopcOutputRequestV1::ArtifactWithDocumentation {
            artifact: path("lib.slib"),
            documentation: path("lib.json"),
        },
    ] {
        let request = ScoopcRequestEnvelopeV1::new(
            RequestCorrelationId::from_array([19; 16]),
            build(output.clone(), manifest(), StageDumpPolicyV1::None).unwrap(),
        );
        let decoded =
            crate::decode_request_frame(&crate::encode_request_frame(&request).unwrap()).unwrap();
        assert_eq!(decoded, request);
        assert_eq!(decoded.build().output(), &output);
        assert_eq!(
            decoded.build().output().artifact().is_none(),
            matches!(output, ScoopcOutputRequestV1::Documentation(_))
        );
    }
}

#[test]
fn documentation_requires_production_manifest_and_a_real_frontend_result() {
    assert_eq!(
        build(
            documentation(),
            CurrentConeRequestV1::SingleFile {
                source: path("main.scoop"),
            },
            StageDumpPolicyV1::None,
        )
        .unwrap_err(),
        ProtocolValidationError::DocumentationRequiresProductionManifest
    );
    let request = build(documentation(), manifest(), dumps(StageDumpKindV1::Hir)).unwrap();
    assert_eq!(
        request
            .clone()
            .with_purpose(CompilationPurpose::Test)
            .unwrap_err(),
        ProtocolValidationError::DocumentationRequiresProductionManifest
    );
    assert_eq!(
        ScoopcRequestEnvelopeV1::new(RequestCorrelationId::from_array([19; 16]), request)
            .observe(StageDumpSet::one(StageDumpKindV1::Hir))
            .unwrap_err(),
        ProtocolValidationError::DocumentationObservation
    );
    for stage in [StageDumpKindV1::Ast, StageDumpKindV1::Hir] {
        assert!(build(documentation(), manifest(), dumps(stage)).is_ok());
    }
    for stage in [StageDumpKindV1::Mir, StageDumpKindV1::Lir] {
        assert_eq!(
            build(documentation(), manifest(), dumps(stage)).unwrap_err(),
            ProtocolValidationError::DocumentationMachineDump
        );
        assert!(
            build(
                ScoopcOutputRequestV1::ArtifactWithDocumentation {
                    artifact: path("lib.slib"),
                    documentation: path("lib.json"),
                },
                manifest(),
                dumps(stage),
            )
            .is_ok()
        );
    }
}
