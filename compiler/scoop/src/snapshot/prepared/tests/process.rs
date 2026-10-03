mod observations;
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
