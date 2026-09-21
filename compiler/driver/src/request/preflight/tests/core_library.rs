use super::*;
use crate::{HostArtifactLocator, normalize_direct_build_request};

mod aliases;
mod metadata;

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
    std::fs::write(
        source.join("src/user_metadata.scoop"),
        include_str!("../../../../../../tests/fixtures/core-library/metadata.scoop"),
    )
    .unwrap();
    std::fs::write(
        source.join("src/user_aliases.scoop"),
        include_str!("../../../../../../tests/fixtures/core-library/type-aliases.scoop"),
    )
    .unwrap();
    let artifact = workspace.path().join("user-library.slib");
    let first = build_core(&source, &artifact);
    let first_dependency = first.artifact().validation().dependency_record();
    let first_fingerprint = first.artifact().validation().artifact_fingerprint();
    assert_explicit_core_version_is_checked(&target, workspace.path(), &artifact);
    let consumer_source = workspace.path().join("consumer.scoop");
    std::fs::write(&consumer_source, CONSUMER).unwrap();
    let consumer_artifact = workspace.path().join("consumer.slib");
    let first_consumer = build_consumer(&target, &consumer_source, &consumer_artifact, &artifact);
    assert_core_views_share_the_dependency_closure(&target, &consumer_source, &artifact);
    let shadow_source = workspace.path().join("shadow.scoop");
    std::fs::write(
        &shadow_source,
        include_str!("../../../../../../tests/fixtures/core-library/prelude-priority.scoop"),
    )
    .unwrap();
    build_consumer(
        &target,
        &shadow_source,
        &workspace.path().join("shadow.slib"),
        &artifact,
    );
    let call_source = workspace.path().join("calls.scoop");
    std::fs::write(
        &call_source,
        include_str!("../../../../../../tests/fixtures/core-library/call-consumer.scoop"),
    )
    .unwrap();
    build_consumer(
        &target,
        &call_source,
        &workspace.path().join("calls.slib"),
        &artifact,
    );
    aliases::assert_alias_stage_dumps(&target, workspace.path(), &artifact);
    assert_non_core_artifact_is_rejected(&target, &consumer_artifact);
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

fn assert_core_views_share_the_dependency_closure(
    target: &scoop_toolchain::ResolvedTargetProfile,
    source: &Path,
    core: &Path,
) {
    let loaded = SingleConeBuildRequest::new(
        CurrentConeInput::SingleFile {
            source: SingleFileLocator::from_path(source).unwrap(),
        },
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap(),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(core).unwrap()),
        target.clone(),
        SlibOutputDestination::new(source.with_extension("shared-views.slib")).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .load_preflight(DecodeLimits::default())
    .unwrap();
    let mut meter = SlibClosureDecodeMeterV1::new(SlibClosureDecodeLimitsV1::M23_DEFAULT);
    let validated = loaded.validate_inner(Some(&mut meter)).unwrap();
    let ValidatedCurrentConeInput::SingleFile { trusted_core, .. } = validated.current() else {
        panic!("single-file input")
    };
    let ValidatedDependencyInputState::Ordinary { closure, .. } = &validated.dependencies().state
    else {
        panic!("ordinary dependencies")
    };
    let member = closure
        .share_artifact(scoop_identity::ConeIdentity::CORE)
        .unwrap();
    metadata::assert_ordinary_interfaces(member.compile().production());
    let world = closure.semantic().imported_semantic_world().unwrap();
    let core = world
        .direct_provider(scoop_identity::ConeIdentity::CORE)
        .unwrap();
    for name in [
        "UserCoreValue",
        "UserCoreAlias",
        "USER_CORE_DEFAULT",
        "userCoreOffset",
    ] {
        assert!(
            core.public_bindings()
                .iter()
                .any(|binding| binding.key().name().as_str() == name)
        );
    }
    assert!(std::ptr::eq(trusted_core.compile(), member.compile()));
    assert!(std::ptr::eq(
        trusted_core.defined_symbols(),
        member.link().defined_symbols()
    ));
    assert_eq!(closure.artifact_count(), 1);
    assert!(!validated.dependencies().is_empty());
    assert!(
        closure
            .share_artifact(scoop_identity::ConeIdentity::SINGLE_FILE)
            .is_none()
    );
}

fn assert_explicit_core_version_is_checked(
    target: &scoop_toolchain::ResolvedTargetProfile,
    workspace: &Path,
    core: &Path,
) {
    let root = workspace.join("wrong-core-version");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("Cone.toml"), "schema = 1\n[cone]\ngroup = \"test\"\nname = \"consumer\"\nversion = \"1.0.0\"\nkind = \"library\"\n[dependencies]\n\"scoop:scoop.core\" = \"0.2.0\"\n").unwrap();
    let error = SingleConeBuildRequest::new(
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(&root),
        },
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap(),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(core).unwrap()),
        target.clone(),
        SlibOutputDestination::new(workspace.join("wrong-version.slib")).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .load_preflight(DecodeLimits::default())
    .unwrap()
    .validate()
    .err()
    .expect("the explicit core version must match the supplied artifact");
    assert!(
        matches!(error, CoreOnlyRequestValidationError::ExplicitDependencies(source)
        if matches!(source.as_ref(), ExplicitDependencyValidationError::ManifestDirectSet { declared, actual }
            if declared.iter().any(|coordinate| coordinate.version() == "0.2.0")
                && actual == &[scoop_identity::ConeCoordinate::reserved_core()]))
    );
    assert!(!root.join("src").exists());
}

fn assert_non_core_artifact_is_rejected(
    target: &scoop_toolchain::ResolvedTargetProfile,
    artifact: &Path,
) {
    let root = artifact.parent().unwrap().join("non-core-input");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("Cone.toml"), "schema = 1\n[cone]\ngroup = \"test\"\nname = \"non-core-input\"\nversion = \"1.0.0\"\nkind = \"library\"\n").unwrap();
    let loaded = SingleConeBuildRequest::new(
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(&root),
        },
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap(),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(artifact).unwrap()),
        target.clone(),
        SlibOutputDestination::new(root.join("consumer.slib")).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .load_preflight(DecodeLimits::default())
    .unwrap();
    assert!(matches!(loaded.validate(),
        Err(CoreOnlyRequestValidationError::ExplicitDependencies(source))
            if matches!(source.as_ref(), ExplicitDependencyValidationError::UnsupportedArtifactShape { .. })));
    assert!(!root.join("src").exists());
}

fn build_consumer(
    target: &scoop_toolchain::ResolvedTargetProfile,
    source: &Path,
    output: &Path,
    core: &Path,
) -> SingleConeProductionSuccess {
    build_consumer_emitting(target, source, output, core, StageDumpPolicy::None)
}

fn build_consumer_emitting(
    target: &scoop_toolchain::ResolvedTargetProfile,
    source: &Path,
    output: &Path,
    core: &Path,
    emit: StageDumpPolicy,
) -> SingleConeProductionSuccess {
    SingleConeBuildRequest::new(
        CurrentConeInput::SingleFile {
            source: SingleFileLocator::from_path(source).unwrap(),
        },
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap(),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(core).unwrap()),
        target.clone(),
        SlibOutputDestination::new(output).unwrap(),
        DiagnosticOutputPolicy::Human,
        emit,
    )
    .unwrap()
    .build_and_publish(DecodeLimits::default())
    .unwrap()
}
