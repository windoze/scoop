use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use scoop_hir::CanonicalHirFoundation;
use scoop_identity::{ArtifactCapabilityProfileId, ConeCoordinate, ConeIdentity};
use scoop_lir::{CanonicalLirFoundation, ValidatedLirTargetSelection};
use scoop_manifest::{ManifestRootLocator, SingleFileLocator};
use scoop_mir::CanonicalMirFoundation;
use scoop_protocol::{
    CurrentConeRequestV1, DiagnosticOriginV1, DiagnosticOutputPolicyV1, DiagnosticSeverityV1,
    ProtocolArtifactFingerprint, ProtocolCodeFingerprint, ProtocolConeIdentity,
    ProtocolHirFingerprint, ProtocolLirFingerprint, ProtocolMirFingerprint,
    ProtocolRuntimeImageFingerprint, RequestCorrelationId, ScoopcRequestEnvelopeV1,
    ScoopcResponseEnvelopeV1, ScoopcSuccessV1, StageDumpPolicyV1, StructuredDiagnosticV1,
    TargetSelectionRequestV1, TrustedCoreRequestV1, encode_machine_capability_frame,
};
use scoop_slib::{
    ConeKind, ConeRecord, ConeSourceForm, DependencyRecord, IdentityFoundationArtifact,
    IdentityFoundationArtifactInput, ProducerRecord, probe_prebuilt_manifest_summary,
};
use scoop_wire::{DecodeLimits, encode};

use super::*;
use crate::{
    ArtifactCacheRoot, BuildGraphExecutionError, BuildGraphOutcome, BuildGraphRequest,
    BuildLimitsProfileV1, BuildRootInput, ChildIoPlan, ChildTransportError, CompletedNodeOrigin,
    DiagnosticsPolicy, OrdinarySourceExecutionError, PairedScoopcLocator,
    ProductionSingleConeCompilerRunner, ResolvedPairedScoopc, SingleConeCompilerRunner,
    TrustedCoreCompletionError, TrustedCoreSlotReceiptBodyV1, TrustedCoreSlotReceiptV1,
    TrustedSysrootRoot,
};

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

struct InvalidArtifactSuccessRunner;

impl SingleConeCompilerRunner for InvalidArtifactSuccessRunner {
    fn invoke(
        &mut self,
        _tool: &ResolvedPairedScoopc,
        request: &ScoopcRequestEnvelopeV1,
        _io: &ChildIoPlan,
    ) -> Result<ScoopcResponseEnvelopeV1, ChildTransportError> {
        std::fs::write(
            request.build().out_slib().to_path_buf().unwrap(),
            foundation_core_artifact(),
        )
        .unwrap();
        Ok(test_success(request.request_id()))
    }
}

struct SourceChangingSuccessRunner {
    source: std::path::PathBuf,
}

#[derive(Default)]
struct RecordingProductionRunner {
    current: Vec<CurrentConeRequestV1>,
    production: ProductionSingleConeCompilerRunner,
}

impl SingleConeCompilerRunner for RecordingProductionRunner {
    fn invoke(
        &mut self,
        tool: &ResolvedPairedScoopc,
        request: &ScoopcRequestEnvelopeV1,
        io: &ChildIoPlan,
    ) -> Result<ScoopcResponseEnvelopeV1, ChildTransportError> {
        self.current.push(request.build().current().clone());
        self.production.invoke(tool, request, io)
    }
}

impl SingleConeCompilerRunner for SourceChangingSuccessRunner {
    fn invoke(
        &mut self,
        _tool: &ResolvedPairedScoopc,
        request: &ScoopcRequestEnvelopeV1,
        _io: &ChildIoPlan,
    ) -> Result<ScoopcResponseEnvelopeV1, ChildTransportError> {
        std::fs::write(&self.source, "class Any\nclass Unit\n").unwrap();
        Ok(test_success(request.request_id()))
    }
}

fn test_success(request_id: RequestCorrelationId) -> ScoopcResponseEnvelopeV1 {
    ScoopcResponseEnvelopeV1::success(
        request_id,
        ScoopcSuccessV1::new(
            ProtocolArtifactFingerprint::from_array([1; 32]),
            ProtocolConeIdentity::from_array(*ConeIdentity::CORE.as_array()),
            ProtocolHirFingerprint::from_array([2; 32]),
            ProtocolMirFingerprint::from_array([3; 32]),
            ProtocolLirFingerprint::from_array([4; 32]),
            ProtocolCodeFingerprint::from_array([5; 32]),
            ProtocolRuntimeImageFingerprint::from_array([6; 32]),
            Vec::new(),
            Vec::new(),
        )
        .unwrap(),
    )
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

fn copy_real_core(sysroot: &Path) {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    let source = workspace.join("sysroot/lib/scoop.core");
    let destination = sysroot.join("lib/scoop.core");
    std::fs::create_dir_all(destination.join("src")).unwrap();
    std::fs::copy(source.join("Cone.toml"), destination.join("Cone.toml")).unwrap();
    for entry in std::fs::read_dir(source.join("src")).unwrap() {
        let entry = entry.unwrap();
        assert!(entry.file_type().unwrap().is_file());
        std::fs::copy(
            entry.path(),
            destination.join("src").join(entry.file_name()),
        )
        .unwrap();
    }
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
        TargetSelectionRequestV1::new("aarch64-apple-darwin".into()).unwrap(),
        PairedScoopcLocator::new(compiler).unwrap(),
        DiagnosticsPolicy::Structured,
        BuildLimitsProfileV1::M23_DEFAULT,
    )
    .unwrap()
}

fn real_manifest_request(
    root: &Path,
    workspace: &Path,
    sysroot: &Path,
    compiler: &Path,
) -> BuildGraphRequest {
    BuildGraphRequest::new(
        BuildRootInput::manifest(ManifestRootLocator::cone_directory(root)).unwrap(),
        vec![],
        ArtifactCacheRoot::new(workspace.join("cache")).unwrap(),
        TrustedSysrootRoot::new(sysroot).unwrap(),
        TargetSelectionRequestV1::new("aarch64-apple-darwin".into()).unwrap(),
        PairedScoopcLocator::new(compiler).unwrap(),
        DiagnosticsPolicy::Structured,
        BuildLimitsProfileV1::M23_DEFAULT,
    )
    .unwrap()
}

fn real_single_file_request(
    source: &Path,
    workspace: &Path,
    sysroot: &Path,
    compiler: &Path,
) -> BuildGraphRequest {
    BuildGraphRequest::new(
        BuildRootInput::single_file(SingleFileLocator::from_path(source).unwrap()),
        vec![],
        ArtifactCacheRoot::new(workspace.join("cache")).unwrap(),
        TrustedSysrootRoot::new(sysroot).unwrap(),
        TargetSelectionRequestV1::new("aarch64-apple-darwin".into()).unwrap(),
        PairedScoopcLocator::new(compiler).unwrap(),
        DiagnosticsPolicy::Structured,
        BuildLimitsProfileV1::M23_DEFAULT,
    )
    .unwrap()
}

fn assert_manual_scoopc_matches(
    executed: &crate::ExecutedBuildGraph,
    identity: ConeIdentity,
    compiler: &Path,
    sysroot: &Path,
    input: &Path,
    output: &Path,
) {
    let result = std::process::Command::new(compiler)
        .arg("build")
        .arg(input)
        .arg("--out-slib")
        .arg(output)
        .env_clear()
        .env("SCOOP_SYSROOT", sysroot)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "manual scoopc failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        executed
            .completed(identity)
            .unwrap()
            .artifact()
            .snapshot()
            .as_bytes(),
        std::fs::read(output).unwrap()
    );
}

fn single_file_request(source: &Path, workspace: &Path) -> BuildGraphRequest {
    let sysroot = workspace.join("sysroot");
    let compiler = workspace.join("bin/scoopc");
    BuildGraphRequest::new(
        BuildRootInput::single_file(SingleFileLocator::from_path(source).unwrap()),
        vec![],
        ArtifactCacheRoot::new(workspace.join("cache")).unwrap(),
        TrustedSysrootRoot::new(sysroot).unwrap(),
        TargetSelectionRequestV1::new("aarch64-apple-darwin".into()).unwrap(),
        PairedScoopcLocator::new(compiler).unwrap(),
        DiagnosticsPolicy::Structured,
        BuildLimitsProfileV1::M23_DEFAULT,
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

fn foundation_artifact_with_core(coordinate: ConeCoordinate) -> Vec<u8> {
    let hir = CanonicalHirFoundation::empty();
    let mir = CanonicalMirFoundation::empty();
    let lir = CanonicalLirFoundation::empty();
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let seed = IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new("prepare-test-seed").unwrap(),
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        selection,
        &hir,
        &mir,
        &lir,
    ))
    .unwrap();
    let fingerprints =
        probe_prebuilt_manifest_summary(seed.as_bytes(), DecodeLimits::M23_DEFAULT, selection)
            .unwrap()
            .semantic_fingerprints();
    let core = DependencyRecord::new(
        ConeCoordinate::reserved_core(),
        fingerprints.hir(),
        fingerprints.mir(),
        fingerprints.lir(),
    )
    .unwrap();
    IdentityFoundationArtifact::write(
        IdentityFoundationArtifactInput::new(
            ProducerRecord::new("prepare-test").unwrap(),
            ConeRecord::new(coordinate, ConeKind::Library, ConeSourceForm::Manifest).unwrap(),
            selection,
            &hir,
            &mir,
            &lir,
        )
        .with_direct_dependencies(vec![core]),
    )
    .unwrap()
    .as_bytes()
    .to_vec()
}

fn foundation_core_artifact() -> Vec<u8> {
    let hir = CanonicalHirFoundation::empty();
    let mir = CanonicalMirFoundation::empty();
    let lir = CanonicalLirFoundation::empty();
    IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new("prepare-core-test").unwrap(),
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        &hir,
        &mir,
        &lir,
    ))
    .unwrap()
    .as_bytes()
    .to_vec()
}

#[test]
fn prepare_materializes_only_immutable_private_source_inputs() {
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
        prepared.trusted_core_preparation(),
        TrustedCorePreparation::Bootstrap(CoreBootstrapReason::Missing)
    );
    assert_eq!(prepared.decode_usage().source_files, 2);
    let private_root = prepared.source_input_path(root_identity).unwrap();
    assert!(private_root.starts_with(prepared.staging_root()));
    assert_eq!(
        std::fs::read(private_root.join("src/main.scoop")).unwrap(),
        b"fun value(): Int = 1\n"
    );
    assert_eq!(
        std::fs::metadata(private_root.join("src/main.scoop"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o400
    );

    std::fs::write(root.join("src/main.scoop"), "fun value(): Int = 2\n").unwrap();
    assert_eq!(
        prepared
            .source_snapshot(root_identity)
            .unwrap()
            .sources()
            .next()
            .unwrap()
            .as_bytes(),
        b"fun value(): Int = 1\n"
    );
}

#[test]
fn prepare_reprobes_and_materializes_every_prebuilt_candidate() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let coordinate = ConeCoordinate::new("test", "prebuilt", "1.0.0").unwrap();
    let artifact = foundation_artifact_with_core(coordinate.clone());
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

    assert_eq!(candidate.snapshot().as_bytes(), artifact);
    assert!(
        candidate
            .materialized_path()
            .starts_with(prepared.staging_root())
    );
    assert_eq!(
        std::fs::read(candidate.materialized_path()).unwrap(),
        candidate.snapshot().as_bytes()
    );
    assert_eq!(prepared.prebuilt_coordinate(identity), Some(&coordinate));
    assert_eq!(
        prepared.prebuilt_artifact_fingerprint(identity),
        Some(candidate.summary().artifact_fingerprint())
    );
    assert_eq!(
        prepared.decode_usage().artifact_snapshot_bytes,
        artifact.len() as u64
    );
}

#[test]
fn prepare_rejects_artifact_changed_after_discovery() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let artifact_path = workspace.join("prebuilt.slib");
    let coordinate = ConeCoordinate::new("test", "prebuilt", "1.0.0").unwrap();
    let mut artifact = foundation_artifact_with_core(coordinate);
    std::fs::write(&artifact_path, &artifact).unwrap();
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"test:prebuilt\" = { version = \"1.0.0\", artifact = \"../prebuilt.slib\" }\n",
    );
    let resolved = request(&root, workspace)
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap();
    *artifact.last_mut().unwrap() ^= 1;
    std::fs::write(&artifact_path, artifact).unwrap();

    assert!(matches!(
        resolved.prepare(),
        Err(PrepareBuildGraphError::ArtifactSummary { .. })
            | Err(PrepareBuildGraphError::ArtifactSummaryChanged(_))
    ));
}

#[test]
fn prepared_graph_retains_the_core_lock_exclusively() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));

    let prepared = prepare(&root, workspace).unwrap();
    let other = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(prepared.core_lock_path())
        .unwrap();

    assert!(matches!(
        fs4::FileExt::try_lock(&other),
        Err(fs4::TryLockError::WouldBlock)
    ));
}

#[test]
fn valid_but_unreceipted_core_slot_is_snapshotted_but_not_reused() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let layout = scoop_toolchain::TrustedCoreSlotLayoutV1::new(
        &sysroot,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    );
    std::fs::create_dir_all(layout.artifact_root()).unwrap();
    let artifact = foundation_core_artifact();
    std::fs::write(layout.artifact(), &artifact).unwrap();

    let mut prepared = prepare(&root, workspace).unwrap();

    assert_eq!(
        prepared.trusted_core_preparation(),
        TrustedCorePreparation::Bootstrap(CoreBootstrapReason::ReceiptUnavailable)
    );
    let existing = prepared.existing_trusted_core_candidate().unwrap();
    assert_eq!(existing.snapshot().as_bytes(), artifact);
    assert!(
        existing
            .materialized_path()
            .starts_with(prepared.staging_root())
    );
    assert!(matches!(
        prepared.complete_trusted_core_node(),
        Err(TrustedCoreCompletionError::BootstrapRequired)
    ));
}

#[test]
fn core_receipt_binding_requires_the_actual_artifact_to_use_the_strong_profile() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let layout = scoop_toolchain::TrustedCoreSlotLayoutV1::new(
        &sysroot,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    );
    std::fs::create_dir_all(layout.artifact_root()).unwrap();
    std::fs::write(layout.artifact(), foundation_core_artifact()).unwrap();

    let prepared = prepare(&root, workspace).unwrap();
    let source_key = prepared.trusted_core_source_key();
    let compiler = prepared.compiler().fingerprint();
    let artifact = prepared
        .existing_trusted_core_candidate()
        .unwrap()
        .summary()
        .artifact_fingerprint();
    let receipt = TrustedCoreSlotReceiptV1::new(
        TrustedCoreSlotReceiptBodyV1::new(
            source_key,
            artifact,
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            compiler,
            ArtifactCapabilityProfileId::single_cone_strong(),
            Vec::new(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(core_receipt_matches(
        &receipt,
        source_key,
        compiler,
        artifact,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        &ArtifactCapabilityProfileId::single_cone_strong(),
    ));
    assert!(!core_receipt_matches(
        &receipt,
        source_key,
        compiler,
        artifact,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        &ArtifactCapabilityProfileId::identity_foundation(),
    ));
    drop(prepared);
    std::fs::write(layout.receipt(), encode(&receipt).unwrap()).unwrap();

    let prepared = prepare(&root, workspace).unwrap();
    assert_eq!(
        prepared.trusted_core_preparation(),
        TrustedCorePreparation::Bootstrap(CoreBootstrapReason::ReceiptUnavailable)
    );
    assert!(prepared.trusted_core_receipt().is_none());
}

#[test]
fn prepared_receipt_cannot_bypass_the_core_dual_view_gate() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let layout = scoop_toolchain::TrustedCoreSlotLayoutV1::new(
        &sysroot,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    );
    std::fs::create_dir_all(layout.artifact_root()).unwrap();
    std::fs::write(layout.artifact(), foundation_core_artifact()).unwrap();

    let mut prepared = prepare(&root, workspace).unwrap();
    let source_key = prepared.trusted_core_source_key();
    let compiler = prepared.compiler().fingerprint();
    let artifact = prepared
        .existing_trusted_core_candidate()
        .unwrap()
        .summary()
        .artifact_fingerprint();
    let receipt = TrustedCoreSlotReceiptV1::new(
        TrustedCoreSlotReceiptBodyV1::new(
            source_key,
            artifact,
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            compiler,
            ArtifactCapabilityProfileId::single_cone_strong(),
            Vec::new(),
        )
        .unwrap(),
    )
    .unwrap();
    match prepared.nodes.get_mut(&ConeIdentity::CORE).unwrap() {
        PreparedGraphNode::TrustedCore(node) => {
            node.preparation = TrustedCorePreparation::ReuseVerifiedSlot;
            node.receipt = Some(receipt);
        }
        _ => unreachable!(),
    }

    assert!(matches!(
        prepared.complete_trusted_core_node(),
        Err(TrustedCoreCompletionError::Artifact(_))
    ));
}

#[test]
fn core_bootstrap_child_failure_never_writes_a_receipt() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let layout = scoop_toolchain::TrustedCoreSlotLayoutV1::new(
        &sysroot,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    );

    let mut prepared = prepare(&root, workspace).unwrap();
    assert!(matches!(
        prepared.execute_trusted_core_bootstrap(
            &mut FailureRunner,
            RequestCorrelationId::from_array([31; 16]),
        ),
        Err(CoreBootstrapExecutionError::ChildFailure(diagnostics))
            if diagnostics.len() == 1
    ));
    assert!(!layout.receipt().exists());
}

#[test]
fn serial_scheduler_stops_before_dependent_after_core_failure() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let prepared = prepare(&root, workspace).unwrap();
    let mut runner = RecordingFailureRunner::default();

    assert!(matches!(
        prepared.execute_with_runner(&mut runner),
        Err(BuildGraphExecutionError::CoreBootstrap(source))
            if matches!(source.as_ref(), CoreBootstrapExecutionError::ChildFailure(_))
    ));
    assert_eq!(
        runner.current,
        vec![CurrentConeRequestV1::TrustedCoreBootstrap]
    );
}

#[test]
fn real_process_bootstrap_then_reuses_core_and_source_cache() {
    let Some(compiler) = std::env::var_os("SCOOP_TEST_PAIRED_SCOOPC") else {
        return;
    };
    let compiler = std::path::PathBuf::from(compiler);
    assert!(compiler.is_absolute());
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    copy_real_core(&sysroot);
    write_manifest(&root, "root", "");

    let build_request = || real_manifest_request(&root, workspace, &sysroot, &compiler);

    let first = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute()
        .unwrap();
    let root_identity = ConeCoordinate::new("test", "root", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    assert_eq!(
        first.observations().child_invocations(),
        &[ConeIdentity::CORE, root_identity]
    );
    assert_eq!(
        first.completed(ConeIdentity::CORE).unwrap().origin(),
        CompletedNodeOrigin::TrustedCore
    );
    assert_eq!(
        first.completed(root_identity).unwrap().origin(),
        CompletedNodeOrigin::Compiled
    );
    assert_manual_scoopc_matches(
        &first,
        root_identity,
        &compiler,
        &sysroot,
        &root,
        &workspace.join("manual-library.slib"),
    );
    assert!(matches!(
        first.into_outcome(),
        BuildGraphOutcome::Library { .. }
    ));

    let second = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute()
        .unwrap();
    assert!(second.observations().child_invocations().is_empty());
    assert_eq!(
        second.completed(ConeIdentity::CORE).unwrap().origin(),
        CompletedNodeOrigin::TrustedCore
    );
    assert_eq!(
        second.completed(root_identity).unwrap().origin(),
        CompletedNodeOrigin::CacheHit
    );
}

#[test]
fn real_process_builds_and_reuses_manifest_executable() {
    let Some(compiler) = std::env::var_os("SCOOP_TEST_PAIRED_SCOOPC") else {
        return;
    };
    let compiler = std::path::PathBuf::from(compiler);
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    copy_real_core(&sysroot);
    write_manifest_source(&root, "executable", "executable", "fun main() {}\n", "");
    let build_request = || real_manifest_request(&root, workspace, &sysroot, &compiler);
    let root_identity = ConeCoordinate::new("test", "executable", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();

    let first = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute()
        .unwrap();
    assert_eq!(
        first.observations().child_invocations(),
        &[ConeIdentity::CORE, root_identity]
    );
    assert_manual_scoopc_matches(
        &first,
        root_identity,
        &compiler,
        &sysroot,
        &root,
        &workspace.join("manual-executable.slib"),
    );
    assert!(matches!(
        first.into_outcome(),
        BuildGraphOutcome::ExecutableArtifact { .. }
    ));

    let second = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute()
        .unwrap();
    assert!(second.observations().child_invocations().is_empty());
    assert_eq!(
        second.completed(root_identity).unwrap().origin(),
        CompletedNodeOrigin::CacheHit
    );
}

#[test]
fn real_process_builds_and_reuses_single_file() {
    let Some(compiler) = std::env::var_os("SCOOP_TEST_PAIRED_SCOOPC") else {
        return;
    };
    let compiler = std::path::PathBuf::from(compiler);
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let source = workspace.join("input.scoop");
    copy_real_core(&sysroot);
    std::fs::write(&source, "fun main() {}\n").unwrap();
    let build_request = || real_single_file_request(&source, workspace, &sysroot, &compiler);

    let first = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute()
        .unwrap();
    assert_eq!(first.dependency_first().len(), 2);
    assert_eq!(
        first.observations().child_invocations(),
        &[ConeIdentity::CORE, ConeIdentity::SINGLE_FILE]
    );
    assert_manual_scoopc_matches(
        &first,
        ConeIdentity::SINGLE_FILE,
        &compiler,
        &sysroot,
        &source,
        &workspace.join("manual-single-file.slib"),
    );
    assert!(matches!(
        first.into_outcome(),
        BuildGraphOutcome::ExecutableArtifact { .. }
    ));

    let second = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute()
        .unwrap();
    assert!(second.observations().child_invocations().is_empty());
    assert_eq!(
        second
            .completed(ConeIdentity::SINGLE_FILE)
            .unwrap()
            .origin(),
        CompletedNodeOrigin::CacheHit
    );
}

#[test]
fn real_process_caches_upstream_before_non_core_capability_failure() {
    let Some(compiler) = std::env::var_os("SCOOP_TEST_PAIRED_SCOOPC") else {
        return;
    };
    let compiler = std::path::PathBuf::from(compiler);
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let dependency = workspace.join("dependency");
    let root = workspace.join("root");
    copy_real_core(&sysroot);
    write_manifest(&dependency, "dependency", "");
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"test:dependency\" = { version = \"1.0.0\", path = \"../dependency\" }\n",
    );
    let dependency_identity = ConeCoordinate::new("test", "dependency", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let root_identity = ConeCoordinate::new("test", "root", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let build_request = || real_manifest_request(&root, workspace, &sysroot, &compiler);

    let mut first_runner = RecordingProductionRunner::default();
    let first_error = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute_with_runner(&mut first_runner)
        .unwrap_err();
    assert_non_core_capability_failure(first_error, root_identity);
    assert_eq!(first_runner.current.len(), 3);
    assert_eq!(
        first_runner.current[0],
        CurrentConeRequestV1::TrustedCoreBootstrap
    );
    assert_manifest_current_identity(&first_runner.current[1], dependency_identity);
    assert_manifest_current_identity(&first_runner.current[2], root_identity);

    let mut second_runner = RecordingProductionRunner::default();
    let second_error = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute_with_runner(&mut second_runner)
        .unwrap_err();
    assert_non_core_capability_failure(second_error, root_identity);
    assert_eq!(second_runner.current.len(), 1);
    assert!(matches!(
        second_runner.current[0],
        CurrentConeRequestV1::ManifestRoot { .. }
    ));
    assert_manifest_current_identity(&second_runner.current[0], root_identity);
}

#[test]
fn real_process_diamond_invokes_shared_core_once_in_canonical_order() {
    let Some(compiler) = std::env::var_os("SCOOP_TEST_PAIRED_SCOOPC") else {
        return;
    };
    let compiler = std::path::PathBuf::from(compiler);
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let alpha = workspace.join("alpha");
    let beta = workspace.join("beta");
    let root = workspace.join("root");
    copy_real_core(&sysroot);
    write_manifest(&alpha, "alpha", "");
    write_manifest(&beta, "beta", "");
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\
         \"test:beta\" = { version = \"1.0.0\", path = \"../beta\" }\n\
         \"test:alpha\" = { version = \"1.0.0\", path = \"../alpha\" }\n",
    );
    let alpha_identity = ConeCoordinate::new("test", "alpha", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let beta_identity = ConeCoordinate::new("test", "beta", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let root_identity = ConeCoordinate::new("test", "root", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let build_request = || real_manifest_request(&root, workspace, &sysroot, &compiler);

    let mut first_runner = RecordingProductionRunner::default();
    let first_error = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute_with_runner(&mut first_runner)
        .unwrap_err();
    assert_non_core_capability_failure(first_error, root_identity);
    assert_eq!(first_runner.current.len(), 4);
    assert_eq!(
        first_runner.current[0],
        CurrentConeRequestV1::TrustedCoreBootstrap
    );
    assert_manifest_current_identity(&first_runner.current[1], alpha_identity);
    assert_manifest_current_identity(&first_runner.current[2], beta_identity);
    assert_manifest_current_identity(&first_runner.current[3], root_identity);

    let mut second_runner = RecordingProductionRunner::default();
    let second_error = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute_with_runner(&mut second_runner)
        .unwrap_err();
    assert_non_core_capability_failure(second_error, root_identity);
    assert_eq!(second_runner.current.len(), 1);
    assert_manifest_current_identity(&second_runner.current[0], root_identity);
}

fn assert_non_core_capability_failure(error: BuildGraphExecutionError, root: ConeIdentity) {
    assert!(matches!(
        error,
        BuildGraphExecutionError::Ordinary(identity, source)
            if identity == root
                && matches!(
                    source.as_ref(),
                    OrdinarySourceExecutionError::ChildFailure(diagnostics)
                        if diagnostics.iter().any(|diagnostic|
                            diagnostic.code()
                                == "SCOOPC_CAPABILITY_NON_CORE_DEPENDENCY_UNAVAILABLE")
                )
    ));
}

fn assert_manifest_current_identity(current: &CurrentConeRequestV1, identity: ConeIdentity) {
    let CurrentConeRequestV1::ManifestRoot { root } = current else {
        panic!("expected manifest request, found {current:?}");
    };
    let expected = identity.to_string();
    assert_eq!(
        root.to_path_buf().unwrap().file_name().unwrap(),
        std::ffi::OsStr::new(&expected)
    );
}

#[test]
fn core_bootstrap_source_change_precedes_output_authority() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let layout = scoop_toolchain::TrustedCoreSlotLayoutV1::new(
        &sysroot,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    );
    let mut runner = SourceChangingSuccessRunner {
        source: sysroot.join("lib/scoop.core/src/core.scoop"),
    };

    let mut prepared = prepare(&root, workspace).unwrap();
    assert!(matches!(
        prepared.execute_trusted_core_bootstrap(
            &mut runner,
            RequestCorrelationId::from_array([32; 16]),
        ),
        Err(CoreBootstrapExecutionError::SourceChanged { .. })
    ));
    assert!(!layout.receipt().exists());
}

#[test]
fn core_bootstrap_invalid_output_never_writes_a_receipt() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let layout = scoop_toolchain::TrustedCoreSlotLayoutV1::new(
        &sysroot,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    );

    let mut prepared = prepare(&root, workspace).unwrap();
    assert!(matches!(
        prepared.execute_trusted_core_bootstrap(
            &mut InvalidArtifactSuccessRunner,
            RequestCorrelationId::from_array([33; 16]),
        ),
        Err(CoreBootstrapExecutionError::Completion(
            TrustedCoreCompletionError::Artifact(_)
        ))
    ));
    assert!(!layout.receipt().exists());
}

#[test]
fn core_source_key_changes_with_the_locked_source_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));

    let prepared = prepare(&root, workspace).unwrap();
    let initial = prepared.trusted_core_source_key();
    drop(prepared);
    std::fs::write(
        sysroot.join("lib/scoop.core/src/core.scoop"),
        "class Any\nclass Unit\n",
    )
    .unwrap();

    let prepared = prepare(&root, workspace).unwrap();
    assert_ne!(prepared.trusted_core_source_key(), initial);
}

#[test]
fn single_file_root_is_materialized_with_its_fixed_semantic_name() {
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
    assert_eq!(input.file_name().unwrap(), "main.scoop");
    assert_eq!(std::fs::read(input).unwrap(), b"fun main() {}\n");
    assert_eq!(
        prepared
            .single_file_snapshot(ConeIdentity::SINGLE_FILE)
            .unwrap()
            .source()
            .identity()
            .logical_path()
            .as_str(),
        "main.scoop"
    );
}

#[test]
fn compile_cache_key_excludes_locator_and_manifest_presentation() {
    let temp = tempfile::tempdir().unwrap();
    let first_workspace = temp.path().join("first");
    let second_workspace = temp.path().join("second");
    let changed_workspace = temp.path().join("changed");

    for workspace in [&first_workspace, &second_workspace, &changed_workspace] {
        let sysroot = workspace.join("sysroot");
        let root = workspace.join("root");
        write_core(&sysroot);
        write_manifest(&root, "root", "");
        write_fake_compiler(&workspace.join("bin/scoopc"));
    }
    std::fs::write(
        second_workspace.join("sysroot/lib/scoop.core/Cone.toml"),
        "# presentation-only comment\nschema=1\n[cone]\ngroup=\"scoop\"\nname=\"scoop.core\"\nversion=\"0.1.0\"\nkind=\"library\"\n",
    )
    .unwrap();
    std::fs::write(
        changed_workspace.join("sysroot/lib/scoop.core/src/core.scoop"),
        "class Any { }\n",
    )
    .unwrap();

    let first = prepare(&first_workspace.join("root"), &first_workspace).unwrap();
    let second = prepare(&second_workspace.join("root"), &second_workspace).unwrap();
    let changed = prepare(&changed_workspace.join("root"), &changed_workspace).unwrap();

    let first_key = first.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();
    let second_key = second.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();
    let changed_key = changed.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();
    assert_eq!(first_key, second_key);
    assert_ne!(first_key, changed_key);
}

#[test]
fn trusted_core_child_plan_is_the_closed_bootstrap_request() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    write_core(&sysroot);
    write_manifest(&root, "root", "");
    write_fake_compiler(&workspace.join("bin/scoopc"));
    let prepared = prepare(&root, workspace).unwrap();
    let request_id = RequestCorrelationId::from_array([4; 16]);

    let plan = prepared
        .child_invocation_plan(ConeIdentity::CORE, request_id, &[])
        .unwrap();

    assert_eq!(plan.identity(), ConeIdentity::CORE);
    assert_eq!(plan.output_path(), prepared.trusted_core_artifact_slot());
    assert_eq!(plan.request().request_id(), request_id);
    assert!(matches!(
        plan.request().build().current(),
        CurrentConeRequestV1::TrustedCoreBootstrap
    ));
    assert!(plan.request().build().direct_slibs().is_empty());
    assert!(plan.request().build().support_slibs().is_empty());
    assert!(matches!(
        plan.request().build().trusted_core(),
        TrustedCoreRequestV1::Bootstrap
    ));
    assert_eq!(
        plan.request().build().diagnostics(),
        DiagnosticOutputPolicyV1::Structured
    );
    assert_eq!(plan.request().build().emit(), StageDumpPolicyV1::None);
    assert_eq!(
        plan.request().build().target().canonical_triple(),
        "aarch64-apple-darwin"
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
