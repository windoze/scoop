use std::{fs, path::Path, process::Command};

use scoop_identity::ConeCoordinate;
use scoop_protocol::{
    CompilationPurpose, HostPathCarrier,
    package::{PackageMemberSelectionV1, PrepareRequestV1, PreparedConeV1, PreparedWorkspaceV1},
};

use super::support::*;

fn host(path: &Path) -> HostPathCarrier {
    HostPathCarrier::from_path(path).unwrap()
}

pub(super) fn request(root: &Path, input: &Path) -> PrepareRequestV1 {
    PrepareRequestV1 {
        version: 1,
        cwd: host(root),
        input: Some(host(input)),
        members: PackageMemberSelectionV1::Default,
        purpose: CompilationPurpose::Production,
        target: "aarch64-apple-darwin".into(),
        sysroot: host(&root.join("sysroot")),
        artifact_roots: Vec::new(),
        package_cache: Some(host(&root.join("cache"))),
        lockfile: None,
        locked: false,
        offline: true,
    }
}

pub(super) fn invoke(root: &Path, request: &PrepareRequestV1) -> std::process::Output {
    command(root, request).output().unwrap()
}

pub(super) fn command(root: &Path, request: &PrepareRequestV1) -> Command {
    let request_file = root.join("prepare-request.json");
    fs::write(&request_file, serde_json::to_vec(request).unwrap()).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_scoop-pm"));
    command
        .current_dir(root)
        .arg("prepare")
        .arg("--request")
        .arg(request_file)
        .arg("--response")
        .arg(root.join("prepared.json"));
    command
}

pub(super) fn prepared(root: &Path, request: &PrepareRequestV1) -> PreparedWorkspaceV1 {
    assert!(success(invoke(root, request)).is_empty());
    let response: PreparedWorkspaceV1 =
        serde_json::from_slice(&fs::read(root.join("prepared.json")).unwrap()).unwrap();
    response.validate().unwrap();
    response
}

#[test]
fn prepare_binds_each_root_and_keeps_inherited_diagnostic_locations() {
    let directory = tempfile::tempdir().unwrap();
    #[cfg(target_os = "linux")]
    let root = {
        use std::os::unix::ffi::OsStringExt;
        directory
            .path()
            .join(std::ffi::OsString::from_vec(vec![b'w', 0xff]))
    };
    #[cfg(not(target_os = "linux"))]
    let root = directory.path().join("workspace");
    fs::create_dir(&root).unwrap();
    core(&root);
    write(
        &root.join("Cone.toml"),
        "schema=1\n[workspace]\nmembers=['first','second']\n[workspace.cone]\ngroup='org.example'\nversion='1.0.0'\nlicense='MIT'\n[workspace.dependencies]\n'org.example:json'={path='json1', version='1.0.0'}\n[overrides]\n'scoop:scoop.core'={path='core'}\n",
    );
    for (name, dependency) in [
        ("first", "{workspace=true}"),
        ("second", "{path='../json2', version='2.0.0'}"),
    ] {
        write(
            &root.join(name).join("Cone.toml"),
            &format!(
                "schema=1\n[cone]\ngroup={{workspace=true}}\nname='{name}'\nversion={{workspace=true}}\nkind='executable'\nlicense={{workspace=true}}\n[dependencies]\n'org.example:json'={dependency}\n"
            ),
        );
    }
    write(&root.join("json1/Cone.toml"), &cone("json", "1.3.0"));
    write(&root.join("json2/Cone.toml"), &cone("json", "2.4.0"));
    let mut request = request(&root, &root);
    request.members = PackageMemberSelectionV1::All;
    let first = prepared(&root, &request);
    assert_eq!(
        first.workspace.to_path_buf().unwrap(),
        fs::canonicalize(&root).unwrap()
    );
    assert_eq!(first.resolutions.len(), 2);
    for resolution in &first.resolutions {
        let node = resolution
            .nodes
            .iter()
            .find(|node| node.cone.coordinate() == &resolution.coordinate)
            .unwrap();
        let PreparedConeV1::Source { config } = &node.cone else {
            panic!()
        };
        let (key, dependency) = config.manifest.semantic().dependency_iter().next().unwrap();
        assert_eq!(
            dependency.version(),
            if resolution.root == "first" {
                "1.3.0"
            } else {
                "2.4.0"
            }
        );
        assert_eq!(config.metadata["license"], "MIT");
        assert_eq!(config.manifest.tests().dependency_iter().len(), 0);
        if resolution.root == "first" {
            let span = config
                .manifest
                .diagnostic_spans()
                .dependency(key)
                .unwrap()
                .declaration()
                .range();
            assert_eq!(&config.manifest_text[span], "{workspace=true}");
            let span = config
                .manifest
                .diagnostic_spans()
                .cone_fields()
                .version()
                .range();
            assert_eq!(&config.manifest_text[span], "{workspace=true}");
        }
    }
    let lock_stamp = fs::metadata(root.join("Cone.lock"))
        .unwrap()
        .modified()
        .unwrap();
    request.locked = true;
    assert_eq!(prepared(&root, &request), first);
    assert_eq!(
        fs::metadata(root.join("Cone.lock"))
            .unwrap()
            .modified()
            .unwrap(),
        lock_stamp
    );
    let original_response = fs::read(root.join("prepared.json")).unwrap();
    write(&root.join("json1/Cone.toml"), &cone("json", "3.0.0"));
    failure(invoke(&root, &request), "requires 1.0.0");
    assert_eq!(
        fs::read(root.join("prepared.json")).unwrap(),
        original_response
    );
    request.version = 999;
    failure(
        invoke(&root, &request),
        "unsupported package protocol version",
    );
    assert_eq!(
        fs::read(root.join("prepared.json")).unwrap(),
        original_response
    );
}

#[test]
fn prepare_only_activates_test_configuration_for_the_selected_test_root() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    core(root);
    app(root, "app", "'org.example:lib'={path='../lib'}");
    let app_path = root.join("app/Cone.toml");
    write(
        &app_path,
        &format!(
            "{}\n[test-dependencies]\n'org.example:testing'={{path='../testing'}}\n'scoop:scoop.core'={{path='../core', version='0.1.0'}}\n",
            fs::read_to_string(&app_path).unwrap()
        ),
    );
    write(
        &root.join("lib/Cone.toml"),
        &format!(
            "{}\n[test-dependencies]\n'org.example:unused'={{path='../absent'}}\n",
            cone("lib", "1.0.0")
        ),
    );
    let mut request = request(root, &root.join("app"));
    let response = prepared(root, &request);
    assert_eq!(response.resolutions[0].nodes.len(), 3);
    request.purpose = CompilationPurpose::Test;
    failure(invoke(root, &request), "testing");
    write(&root.join("testing/Cone.toml"), &cone("testing", "1.0.0"));
    let response = prepared(root, &request);
    let graph = &response.resolutions[0];
    assert_eq!(graph.nodes.len(), 4);
    for node in &graph.nodes {
        let PreparedConeV1::Source { config } = &node.cone else {
            panic!()
        };
        let expected = usize::from(node.cone.coordinate().name() == "app");
        assert_eq!(node.test_dependencies.len(), expected);
        assert_eq!(config.manifest.tests().dependency_iter().len(), expected);
    }
    let mut invalid = response;
    invalid.resolutions[0].nodes.pop();
    assert!(
        invalid
            .validate()
            .unwrap_err()
            .contains("missing prepared node")
    );
}

#[test]
fn prepare_transports_prebuilt_summaries_and_rejects_mismatched_edges() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let core = super::artifact::artifact(
        &root.join("sysroot/lib/aarch64-apple-darwin/scoop.core.slib"),
        ConeCoordinate::reserved_core(),
        vec![],
        1,
    );
    let library = ConeCoordinate::new("org.example", "codec", "1.0.0").unwrap();
    super::artifact::artifact(&root.join("codec.slib"), library, vec![core.clone()], 2);
    write(
        &root.join("app/Cone.toml"),
        &format!(
            "{}\n[dependencies]\n'org.example:codec'={{artifact='../codec.slib', version='=1.0.0'}}\n",
            cone("app", "1.0.0")
        ),
    );
    let mut response = prepared(root, &request(root, &root.join("app")));
    let node = response.resolutions[0]
        .nodes
        .iter_mut()
        .find(|node| node.cone.coordinate().name() == "codec")
        .unwrap();
    let PreparedConeV1::Prebuilt { artifact, summary } = &node.cone else {
        panic!()
    };
    assert_eq!(
        artifact.to_path_buf().unwrap(),
        fs::canonicalize(root.join("codec.slib")).unwrap()
    );
    assert_eq!(summary.direct_dependencies(), &[core]);
    node.dependencies.clear();
    assert!(
        response
            .validate()
            .unwrap_err()
            .contains("edges do not match")
    );
}
