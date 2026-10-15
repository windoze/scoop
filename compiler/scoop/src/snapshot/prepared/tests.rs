use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_manifest::{ManifestRootLocator, SingleFileLocator};
use scoop_protocol::{
    CurrentConeRequestV1, DiagnosticOriginV1, DiagnosticOutputPolicyV1, DiagnosticSeverityV1,
    RequestCorrelationId, ScoopcRequestEnvelopeV1, ScoopcResponseEnvelopeV1, StageDumpPolicyV1,
    StructuredDiagnosticV1, TargetSelectionRequestV1, TrustedCoreRequestV1,
    encode_machine_capability_frame,
};
use scoop_slib::{
    ConeKind, ConeRecord, ConeSourceForm, DependencyRecord, read_artifact_manifest_summary,
};

use super::*;
use crate::{
    ArtifactCacheRoot, BuildGraphExecutionError, BuildGraphRequest, BuildRootInput, ChildIoPlan,
    ChildTransportError, CompileCacheStoreV1, DiagnosticsPolicy, PairedScoopcLocator,
    ResolvedPairedScoopc, SingleConeCompilerRunner, TrustedSysrootRoot,
};

mod core;
mod observation_locks;
mod testing;

struct FailureRunner;

impl SingleConeCompilerRunner for FailureRunner {
    fn invoke(
        &mut self,
        _tool: &ResolvedPairedScoopc,
        request: &ScoopcRequestEnvelopeV1,
        _io: &ChildIoPlan,
    ) -> Result<ScoopcResponseEnvelopeV1, ChildTransportError> {
        let diagnostic = StructuredDiagnosticV1::new(
            DiagnosticSeverityV1::Error,
            "SCOOPC_TEST_CORE_FAILURE".to_owned(),
            "core failed".to_owned(),
            DiagnosticOriginV1::None,
            Vec::new(),
        )
        .unwrap();
        Ok(ScoopcResponseEnvelopeV1::failure(request.request_id(), vec![diagnostic]).unwrap())
    }
}

#[derive(Default)]
struct RecordingFailureRunner {
    current: Vec<CurrentConeRequestV1>,
}

impl SingleConeCompilerRunner for RecordingFailureRunner {
    fn invoke(
        &mut self,
        _tool: &ResolvedPairedScoopc,
        request: &ScoopcRequestEnvelopeV1,
        io: &ChildIoPlan,
    ) -> Result<ScoopcResponseEnvelopeV1, ChildTransportError> {
        self.current.push(request.build().current().clone());
        FailureRunner.invoke(_tool, request, io)
    }
}

fn write_manifest(root: &Path, name: &str, dependencies: &str) {
    write_manifest_source(
        root,
        name,
        "library",
        "fun value(): Int = 1\n",
        dependencies,
    );
}

fn write_manifest_source(root: &Path, name: &str, kind: &str, source: &str, dependencies: &str) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        format!(
            "schema = 1\n[cone]\ngroup = \"test\"\nname = \"{name}\"\nversion = \"1.0.0\"\nkind = \"{kind}\"\n{dependencies}"
        ),
    )
    .unwrap();
    std::fs::write(root.join("src/main.scoop"), source).unwrap();
}

fn write_core(sysroot: &Path) {
    let root = sysroot.join("lib/scoop.core");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        "schema = 1\n[cone]\ngroup = \"scoop\"\nname = \"scoop.core\"\nversion = \"0.1.0\"\nkind = \"library\"\n",
    )
    .unwrap();
    std::fs::write(root.join("src/core.scoop"), "class Any\n").unwrap();
}

fn write_fake_compiler(path: &Path) {
    let capability = scoop_toolchain::paired_compiler_machine_capability().unwrap();
    let frame = encode_machine_capability_frame(&capability).unwrap();
    let escaped = frame
        .iter()
        .map(|byte| format!("\\{:03o}", byte))
        .collect::<String>();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\nprintf '{escaped}'\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn request(root: &Path, workspace: &Path) -> BuildGraphRequest {
    let sysroot = workspace.join("sysroot");
    let compiler = workspace.join("bin/scoopc");
    BuildGraphRequest::new(
        BuildRootInput::manifest(ManifestRootLocator::cone_directory(root)).unwrap(),
        vec![],
        ArtifactCacheRoot::new(workspace.join("cache")).unwrap(),
        TrustedSysrootRoot::new(sysroot).unwrap(),
        TargetSelectionRequestV1::new(scoop_toolchain::host_target_triple().unwrap().into())
            .unwrap(),
        PairedScoopcLocator::new(compiler).unwrap(),
        DiagnosticsPolicy::Structured,
    )
    .unwrap()
}

fn single_file_request(source: &Path, workspace: &Path) -> BuildGraphRequest {
    let sysroot = workspace.join("sysroot");
    let compiler = workspace.join("bin/scoopc");
    BuildGraphRequest::new(
        BuildRootInput::single_file(SingleFileLocator::from_path(source).unwrap()),
        vec![],
        ArtifactCacheRoot::new(workspace.join("cache")).unwrap(),
        TrustedSysrootRoot::new(sysroot).unwrap(),
        TargetSelectionRequestV1::new(scoop_toolchain::host_target_triple().unwrap().into())
            .unwrap(),
        PairedScoopcLocator::new(compiler).unwrap(),
        DiagnosticsPolicy::Structured,
    )
    .unwrap()
}

fn prepare(root: &Path, workspace: &Path) -> Result<PreparedBuildGraph, PrepareBuildGraphError> {
    request(root, workspace)
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
}

fn manifest_artifact_with_core(coordinate: ConeCoordinate) -> Vec<u8> {
    let selection = crate::test_artifacts::host_target();
    let seed = crate::test_artifacts::manifest_archive(
        crate::test_artifacts::host_target(),
        scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        "prepare-test-seed",
        Vec::new(),
    );
    let fingerprints = read_artifact_manifest_summary(seed.as_bytes(), selection)
        .unwrap()
        .semantic_fingerprints();
    let core = DependencyRecord::new(
        ConeCoordinate::reserved_core(),
        fingerprints.hir(),
        fingerprints.mir(),
        fingerprints.lir(),
    )
    .unwrap();
    crate::test_artifacts::manifest_archive(
        crate::test_artifacts::host_target(),
        scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ConeRecord::new(coordinate, ConeKind::Library, ConeSourceForm::Manifest).unwrap(),
        "prepare-test",
        vec![core],
    )
    .into_bytes()
}

#[test]
fn prepare_uses_the_actual_source_root() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));

    let prepared = prepare(&root, workspace).unwrap();
    let root_identity = ConeCoordinate::new("test", "root", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();

    assert_eq!(prepared.node_count(), 2);
    assert_eq!(prepared.edge_count(), 1);
    assert_eq!(
        prepared.node_representation(ConeIdentity::CORE),
        Some(PreparedNodeRepresentation::ManifestSource)
    );

    let input_root = prepared.source_input_path(root_identity).unwrap();
    assert_eq!(input_root, root.canonicalize().unwrap());
    assert_eq!(
        std::fs::read(input_root.join("src/main.scoop")).unwrap(),
        b"fun value(): Int = 1\n"
    );
}

#[test]
fn prepare_uses_the_actual_prebuilt_artifact_path() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let coordinate = ConeCoordinate::new("test", "prebuilt", "1.0.0").unwrap();
    let artifact = manifest_artifact_with_core(coordinate.clone());
    std::fs::write(workspace.join("prebuilt.slib"), &artifact).unwrap();
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"test:prebuilt\" = { version = \"1.0.0\", artifact = \"../prebuilt.slib\" }\n",
    );

    let prepared = prepare(&root, workspace).unwrap();
    let identity = coordinate.identity().unwrap();
    let candidate = prepared
        .prebuilt_candidates(identity)
        .unwrap()
        .next()
        .unwrap();

    assert_eq!(candidate.summary().cone().identity(), identity);
    assert_eq!(
        candidate.source_locator(),
        workspace.join("prebuilt.slib").canonicalize().unwrap()
    );
    assert_eq!(prepared.prebuilt_coordinate(identity), Some(&coordinate));
    assert_eq!(
        prepared.prebuilt_artifact_fingerprint(identity),
        Some(candidate.summary().artifact_fingerprint())
    );
}

#[test]
fn single_file_uses_actual_path_and_fixed_semantic_name() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    write_core(&sysroot);
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let source = workspace.join("renamed.scoop");
    std::fs::write(&source, "fun main() {}\n").unwrap();

    let prepared = single_file_request(&source, workspace)
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap();

    let input = prepared
        .source_input_path(ConeIdentity::SINGLE_FILE)
        .unwrap();
    assert_eq!(input, source.canonicalize().unwrap());
    assert_eq!(std::fs::read(input).unwrap(), b"fun main() {}\n");
    assert_eq!(
        prepared
            .single_file_input(ConeIdentity::SINGLE_FILE)
            .unwrap()
            .source()
            .identity()
            .logical_path()
            .as_str(),
        "main.scoop"
    );
}

#[test]
fn compile_cache_key_excludes_manifest_presentation() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let first = prepare(&root, workspace).unwrap();
    let first_key = first.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();
    std::fs::write(
        sysroot.join("lib/scoop.core/Cone.toml"),
        "# presentation-only comment\nschema=1\n[cone]\ngroup=\"scoop\"\nname=\"scoop.core\"\nversion=\"0.1.0\"\nkind=\"library\"\n",
    ).unwrap();
    let second = prepare(&root, workspace).unwrap();
    assert_eq!(
        first_key,
        second.compile_cache_key(ConeIdentity::CORE, &[]).unwrap()
    );
    std::fs::write(
        sysroot.join("lib/scoop.core/src/core.scoop"),
        "class Any { }\n",
    )
    .unwrap();
    let changed = prepare(&root, workspace).unwrap();
    assert_ne!(
        first_key,
        changed.compile_cache_key(ConeIdentity::CORE, &[]).unwrap()
    );
}

#[test]
fn child_request_id_changes_only_the_protocol_envelope() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let prepared = prepare(&root, workspace).unwrap();
    let key_before = prepared.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();

    let first = prepared
        .child_invocation_plan(
            ConeIdentity::CORE,
            RequestCorrelationId::from_array([40; 16]),
            &[],
        )
        .unwrap();
    let second = prepared
        .child_invocation_plan(
            ConeIdentity::CORE,
            RequestCorrelationId::from_array([41; 16]),
            &[],
        )
        .unwrap();

    assert_ne!(first.request().request_id(), second.request().request_id());
    assert_eq!(first.request().build(), second.request().build());
    assert_eq!(first.output_path(), second.output_path());
    assert_eq!(first.io(), second.io());
    assert_eq!(
        key_before,
        prepared.compile_cache_key(ConeIdentity::CORE, &[]).unwrap()
    );
}

#[test]
fn ordinary_child_plan_cannot_precede_trusted_core_completion() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    write_core(&sysroot);
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let source = workspace.join("main.scoop");
    std::fs::write(&source, "fun main() {}\n").unwrap();
    let prepared = single_file_request(&source, workspace)
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap();

    assert!(matches!(
        prepared.child_invocation_plan(
            ConeIdentity::SINGLE_FILE,
            RequestCorrelationId::from_array([5; 16]),
            &[],
        ),
        Err(ChildRequestPlanError::MissingTrustedCore)
    ));
}
