use super::*;
use crate::{HostArtifactLocator, normalize_direct_build_request};

mod aliases;
mod calls;
mod default_data_flow;
mod direct_inputs;
mod equality;
mod intrinsics;
mod member_calls;
mod metadata;
mod source_fields;

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
        source.join("src/user_abi.scoop"),
        include_str!("../../../../../../tests/fixtures/core-library/abi.scoop"),
    )
    .unwrap();
    std::fs::write(
        source.join("src/user_aliases.scoop"),
        include_str!("../../../../../../tests/fixtures/core-library/type-aliases.scoop"),
    )
    .unwrap();
    std::fs::write(
        source.join("src/user_integer_defaults.scoop"),
        include_str!("../../../../../../tests/fixtures/core-library/integer-defaults.scoop"),
    )
    .unwrap();
    std::fs::write(
        source.join("src/user_default_data_flow.scoop"),
        include_str!("../../../../../../tests/fixtures/core-library/default-data-flow.scoop"),
    )
    .unwrap();
    let artifact = workspace.path().join("user-library.slib");
    source_fields::write_source(&source);
    let first = build_core(&source, &artifact);
    direct_inputs::check(&target, workspace.path(), &artifact);
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
    let abi_source = workspace.path().join("abi-calls.scoop");
    std::fs::write(
        &abi_source,
        include_str!("../../../../../../tests/fixtures/core-library/abi-consumer.scoop"),
    )
    .unwrap();
    build_consumer(
        &target,
        &abi_source,
        &workspace.path().join("abi-calls.slib"),
        &artifact,
    );
    aliases::assert_alias_stage_dumps(&target, workspace.path(), &artifact);
    calls::assert_initialization_and_dependency_calls(&target, workspace.path(), &artifact);
    intrinsics::assert_shared_intrinsic_constants(&target, workspace.path(), &artifact);
    member_calls::assert_member_calls(&target, workspace.path(), &artifact);
    equality::assert_equality(&target, workspace.path(), &artifact);
    intrinsics::assert_normalized_integer_defaults(&target, workspace.path(), &artifact);
    default_data_flow::assert_branching_defaults(&target, workspace.path(), &artifact);
    intrinsics::assert_integer_exception_requires_layout(&target, workspace.path(), &artifact);
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
    .build_and_publish()
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
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(core).unwrap()),
        target.clone(),
        SlibOutputDestination::new(source.with_extension("shared-views.slib")).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .load_preflight()
    .unwrap();

    let validated = loaded.validate_inner().unwrap();
    assert!(matches!(
        validated.current(),
        ValidatedCurrentConeInput::SingleFile { .. }
    ));
    let ValidatedCompilerProtocols::Imported(inputs) = validated.protocols() else {
        panic!("single-file input imports its protocols from the shared dependency closure")
    };
    let closure = &validated.dependencies().closure;
    let member = closure
        .share_artifact(scoop_identity::ConeIdentity::CORE)
        .unwrap();
    metadata::assert_ordinary_interfaces(member.compile().production());
    source_fields::assert_source_fields(member.compile().production());
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
    let string = inputs.protocols().fundamental_types().string();
    assert_eq!(string.provider(), member.compile().identity());
    assert_eq!(
        member.compile().hir().identity(string.persistent()),
        Some(string.identity())
    );
    let cycle = inputs
        .protocols()
        .exceptions()
        .initialization_cycle_thrower();
    let scoop_hir::ImportedCoreProtocolCallableDefinition::Function(function) = cycle.definition()
    else {
        panic!("initialization service retains its source function identity")
    };
    assert_eq!(cycle.provider(), member.compile().identity());
    assert_eq!(
        member.compile().hir().identity(function.persistent()),
        Some(function)
    );
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
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(core).unwrap()),
        target.clone(),
        SlibOutputDestination::new(workspace.join("wrong-version.slib")).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .load_preflight()
    .unwrap()
    .validate()
    .err()
    .expect("the explicit core version must match the supplied artifact");
    assert!(
        matches!(error, SingleConeDependencyValidationError::ExplicitDependencies(source)
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
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(artifact).unwrap()),
        target.clone(),
        SlibOutputDestination::new(root.join("consumer.slib")).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .load_preflight()
    .unwrap();
    assert!(matches!(loaded.validate(),
        Err(SingleConeDependencyValidationError::ExplicitDependencies(source))
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
    consumer_request(target, source, output, core, emit)
        .build_and_publish()
        .unwrap()
}

fn consumer_request(
    target: &scoop_toolchain::ResolvedTargetProfile,
    source: &Path,
    output: &Path,
    core: &Path,
    emit: StageDumpPolicy,
) -> SingleConeBuildRequest {
    SingleConeBuildRequest::new(
        CurrentConeInput::SingleFile {
            source: SingleFileLocator::from_path(source).unwrap(),
        },
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(core).unwrap()),
        target.clone(),
        SlibOutputDestination::new(output).unwrap(),
        DiagnosticOutputPolicy::Human,
        emit,
    )
    .unwrap()
}
