use super::*;
use crate::{TrustedCoreArtifactInput, normalize_direct_build_request};

const EXTENSION: &str =
    include_str!("../../../../../../tests/fixtures/core-library/extension.scoop");
const CONSUMER: &str = include_str!("../../../../../../tests/fixtures/core-library/consumer.scoop");

#[test]
fn edited_core_library_builds_from_a_manifest_and_is_consumed_from_any_output_path() {
    let Ok(target) = scoop_toolchain::ResolvedTargetProfile::resolve_host() else {
        return;
    };
    let workspace = tempfile::tempdir().unwrap();
    bootstrap::copy_trusted_core_sources(workspace.path());
    let source = workspace.path().join("edited-library");
    std::fs::rename(workspace.path().join("lib/scoop.core"), &source).unwrap();
    std::fs::write(source.join("src/user_extension.scoop"), EXTENSION).unwrap();
    let artifact = workspace.path().join("user-library.slib");
    let first = build_core(&source, &artifact);
    let first_dependency = first.artifact().validation().dependency_record();
    let first_fingerprint = first.artifact().validation().artifact_fingerprint();
    let consumer_source = workspace.path().join("consumer.scoop");
    std::fs::write(&consumer_source, CONSUMER).unwrap();
    let consumer_artifact = workspace.path().join("consumer.slib");
    let first_consumer = build_consumer(&target, &consumer_source, &consumer_artifact, &artifact);
    assert_eq!(
        first_consumer.artifact().validation().direct_dependencies(),
        &[first_dependency]
    );

    // Rebuilding modified library source uses the same coordinate and ordinary
    // dependency fingerprints; neither source nor artifact lives in a sysroot.
    std::fs::write(
        source.join("src/user_extension.scoop"),
        EXTENSION.replace("+ 1", "+ 2"),
    )
    .unwrap();
    let second = build_core(&source, &artifact);
    assert_ne!(
        second.artifact().validation().artifact_fingerprint(),
        first_fingerprint
    );
    let second_dependency = second.artifact().validation().dependency_record();
    let second_consumer = build_consumer(&target, &consumer_source, &consumer_artifact, &artifact);
    assert_eq!(
        second_consumer
            .artifact()
            .validation()
            .direct_dependencies(),
        &[second_dependency]
    );
    assert_ne!(
        first_consumer
            .artifact()
            .validation()
            .artifact_fingerprint(),
        second_consumer
            .artifact()
            .validation()
            .artifact_fingerprint()
    );
}

fn build_core(source: &Path, artifact: &Path) -> SingleConeProductionSuccess {
    normalize_direct_build_request(
        source,
        Vec::new(),
        Vec::new(),
        artifact,
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .build_and_publish(DecodeLimits::default())
    .unwrap()
}

fn build_consumer(
    target: &scoop_toolchain::ResolvedTargetProfile,
    source: &Path,
    output: &Path,
    core: &Path,
) -> SingleConeProductionSuccess {
    SingleConeBuildRequest::new(
        CurrentConeInput::SingleFile {
            source: SingleFileLocator::from_path(source).unwrap(),
        },
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap(),
        TrustedCoreInput::Artifact(
            TrustedCoreArtifactInput::new(core, target.lir_target_selection()).unwrap(),
        ),
        target.clone(),
        SlibOutputDestination::new(output).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .build_and_publish(DecodeLimits::default())
    .unwrap()
}
