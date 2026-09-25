use super::*;
use crate::{BuildGraphOutcome, CompletedNodeOrigin, ProductionSingleConeCompilerRunner};

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
    )
    .unwrap()
}

fn assert_direct_protocol_matches(
    executed: &crate::ExecutedBuildGraph,
    identity: ConeIdentity,
    compiler: &Path,
    input: &Path,
    output: &Path,
) {
    use scoop_protocol::{HostPathCarrier, ScoopcBuildRequestV1};
    let path = |path: &Path| HostPathCarrier::from_path(path).unwrap();
    let current = if input.is_dir() {
        CurrentConeRequestV1::ManifestRoot { root: path(input) }
    } else {
        CurrentConeRequestV1::SingleFile {
            source: path(input),
        }
    };
    let core = executed.completed(ConeIdentity::CORE).unwrap();
    let request = ScoopcRequestEnvelopeV1::new(
        RequestCorrelationId::from_array([92; 16]),
        ScoopcBuildRequestV1::new(
            current,
            Vec::new(),
            Vec::new(),
            TrustedCoreRequestV1::ArtifactSlot {
                artifact: path(core.materialized_child_path().as_path()),
            },
            TargetSelectionRequestV1::new("aarch64-apple-darwin".into()).unwrap(),
            path(output),
            DiagnosticOutputPolicyV1::Structured,
            StageDumpPolicyV1::None,
        )
        .unwrap(),
    );
    let tool = ResolvedPairedScoopc::resolve(&PairedScoopcLocator::new(compiler).unwrap()).unwrap();
    let result = ProductionSingleConeCompilerRunner
        .invoke(
            &tool,
            &request,
            &ChildIoPlan::new(output.parent().unwrap().join("unused-sysroot")),
        )
        .unwrap();
    assert!(
        matches!(result, ScoopcResponseEnvelopeV1::Success { .. }),
        "{result:?}"
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

#[test]
fn real_process_compiles_then_reuses_core_and_source_cache() {
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
        CompletedNodeOrigin::Compiled
    );
    assert_eq!(
        first.completed(root_identity).unwrap().origin(),
        CompletedNodeOrigin::Compiled
    );
    assert_direct_protocol_matches(
        &first,
        root_identity,
        &compiler,
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
        CompletedNodeOrigin::CacheHit
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
    assert_direct_protocol_matches(
        &first,
        root_identity,
        &compiler,
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
    assert_direct_protocol_matches(
        &first,
        ConeIdentity::SINGLE_FILE,
        &compiler,
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
fn real_process_builds_and_reuses_a_source_dependency() {
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
    write_manifest_source(
        &dependency,
        "dependency",
        "library",
        "package dependency.api\n\npublic fun value(): Int = 1\n",
        "",
    );
    write_manifest_source(
        &root,
        "root",
        "library",
        "package consumer\n\nimport dependency.api.value\n\npublic fun run(): Int = value()\n",
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
    let first = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute_with_runner(&mut first_runner)
        .unwrap();
    assert_eq!(first_runner.current.len(), 3);
    assert_manifest_current_identity(&first_runner.current[0], ConeIdentity::CORE);
    assert_manifest_current_identity(&first_runner.current[1], dependency_identity);
    assert_manifest_current_identity(&first_runner.current[2], root_identity);
    assert_eq!(
        first.completed(root_identity).unwrap().origin(),
        CompletedNodeOrigin::Compiled
    );
    let root_node = first.completed(root_identity).unwrap();
    let artifact = root_node.closure().artifact(root_identity).unwrap();
    assert_eq!(artifact.compile().identity(), root_identity);
    assert_eq!(artifact.link().identity(), root_identity);
    assert!(std::ptr::eq(
        artifact.compile(),
        root_node.artifact().compile()
    ));
    assert!(std::ptr::eq(artifact.link(), root_node.artifact().link()));
    assert!(matches!(
        first.into_outcome(),
        BuildGraphOutcome::Library { .. }
    ));

    let mut second_runner = RecordingProductionRunner::default();
    let second = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute_with_runner(&mut second_runner)
        .unwrap();
    assert!(second_runner.current.is_empty());
    assert_eq!(
        second.completed(dependency_identity).unwrap().origin(),
        CompletedNodeOrigin::CacheHit
    );
    assert_eq!(
        second.completed(root_identity).unwrap().origin(),
        CompletedNodeOrigin::CacheHit
    );
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
    let first = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute_with_runner(&mut first_runner)
        .unwrap();
    assert_eq!(first_runner.current.len(), 4);
    assert_manifest_current_identity(&first_runner.current[0], ConeIdentity::CORE);
    assert_manifest_current_identity(&first_runner.current[1], alpha_identity);
    assert_manifest_current_identity(&first_runner.current[2], beta_identity);
    assert_manifest_current_identity(&first_runner.current[3], root_identity);
    assert!(matches!(
        first.into_outcome(),
        BuildGraphOutcome::Library { .. }
    ));

    let mut second_runner = RecordingProductionRunner::default();
    let second = build_request()
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute_with_runner(&mut second_runner)
        .unwrap();
    assert!(second_runner.current.is_empty());
    assert_eq!(
        second.completed(root_identity).unwrap().origin(),
        CompletedNodeOrigin::CacheHit
    );
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

mod core_cache;
mod core_locator;
mod core_root;
