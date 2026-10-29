use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

use scoop_protocol::{
    HostPathCarrier,
    package::{
        ManifestOverlayV1, ManifestOverlaysV1, PackageMembersV1, PreparedConeV1,
        PreparedWorkspaceV1,
    },
};

use super::{prepare, prepare_cache, support::*};

fn input(manifests: &[(&Path, &str)]) -> ManifestOverlaysV1 {
    ManifestOverlaysV1 {
        version: scoop_protocol::package::PACKAGE_PROTOCOL_VERSION,
        manifests: manifests
            .iter()
            .map(|(path, text)| ManifestOverlayV1 {
                path: HostPathCarrier::from_path(path).unwrap(),
                text: (*text).into(),
            })
            .collect(),
    }
}

fn inspect(root: &Path, command: &str, input: &ManifestOverlaysV1) -> Output {
    let mut process = Command::new(env!("CARGO_BIN_EXE_scoop-pm"));
    process
        .current_dir(root)
        .args([command, "--overlays-stdin", "--json"]);
    if command == "metadata" {
        process
            .args(["--offline", "--target", "aarch64-apple-darwin", "--sysroot"])
            .arg(root.join("sysroot"))
            .arg("--package-cache-dir")
            .arg(root.join("cache"));
    }
    let mut child = process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(input).unwrap())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn source<'a>(
    result: &'a PreparedWorkspaceV1,
    name: &str,
) -> &'a scoop_protocol::package::ExpandedConeInputV1 {
    let node = result.resolutions[0]
        .nodes
        .iter()
        .find(|node| node.cone.coordinate().name() == name)
        .unwrap();
    let PreparedConeV1::Source { config } = &node.cone else {
        panic!()
    };
    config
}

#[test]
fn metadata_uses_unsaved_workspace_and_path_manifests_without_persisting_them() {
    let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = directory.path();
    core(root);
    let workspace = root.join("Cone.toml");
    let owner = "schema=1\n[workspace]\nmembers=['app']\n[workspace.cone]\nlicense='MIT'\n[workspace.dependencies]\n'org.example:lib'={path='lib',version='^1.0.0'}\n[overrides]\n'scoop:scoop.core'={path='core'}\n";
    write(&workspace, owner);
    let member = root.join("app/Cone.toml");
    let app = format!(
        "{}license={{workspace=true}}\n[dependencies]\n'org.example:lib'={{workspace=true}}\n",
        cone("app", "1.0.0")
    );
    write(&member, &app);
    let library = root.join("lib/Cone.toml");
    let lib = cone("lib", "1.2.0");
    write(&library, &lib);
    let request = prepare::request(root, root);
    let original = prepare::prepared(root, &request);
    let lock_path = root.join("Cone.lock");
    let lock_bytes = fs::read(&lock_path).unwrap();
    let lock_stamp = fs::metadata(&lock_path).unwrap().modified().unwrap();
    let records = prepare_cache::records(root);

    let edited_owner = owner.replace("MIT", "Apache-2.0");
    let edited_app = app.replace("schema = 1", "schema = 1\nsources=[{path='alternate'}]");
    let edited_lib = lib.clone() + "description='Unsaved dependency'\n";
    let overlays = input(&[
        (&workspace, &edited_owner),
        (&member, &edited_app),
        (&library, &edited_lib),
    ]);
    let output = success(inspect(root, "metadata", &overlays));
    let edited: PreparedWorkspaceV1 = serde_json::from_str(&output).unwrap();
    edited.validate().unwrap();
    assert_eq!(source(&edited, "app").metadata["license"], "Apache-2.0");
    assert_eq!(source(&edited, "app").manifest_text, edited_app);
    assert_eq!(
        source(&edited, "lib").metadata["description"],
        "Unsaved dependency"
    );
    assert_eq!(
        edited.resolutions[0]
            .nodes
            .iter()
            .map(|node| node.cone.coordinate())
            .collect::<Vec<_>>(),
        original.resolutions[0]
            .nodes
            .iter()
            .map(|node| node.cone.coordinate())
            .collect::<Vec<_>>()
    );

    let conflict = owner.replace("^1.0.0", "^2.0.0");
    prepare::failure(
        inspect(root, "metadata", &input(&[(&workspace, &conflict)])),
        "run scoop pm resolve",
    );
    assert_eq!(fs::read_to_string(&workspace).unwrap(), owner);
    assert_eq!(fs::read_to_string(&member).unwrap(), app);
    assert_eq!(fs::read_to_string(&library).unwrap(), lib);
    assert_eq!(fs::read(&lock_path).unwrap(), lock_bytes);
    assert_eq!(
        fs::metadata(&lock_path).unwrap().modified().unwrap(),
        lock_stamp
    );
    assert_eq!(prepare_cache::records(root), records);
    let restored = success(inspect(root, "metadata", &input(&[])));
    assert_eq!(
        serde_json::from_str::<PreparedWorkspaceV1>(&restored).unwrap(),
        original
    );
}

#[test]
fn members_discover_unsaved_manifests_and_report_buffer_locations_without_a_lock() {
    let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = directory.path();
    let workspace = root.join("Cone.toml");
    let original = "schema=1\n[workspace]\nmembers=[]\n";
    write(&workspace, original);
    fs::create_dir(root.join("new")).unwrap();
    let member = root.join("new/Cone.toml");
    let owner = original.replace("members=[]", "members=['new']");
    let cone = cone("new", "1.0.0");
    let overlays = input(&[(&workspace, &owner), (&member, &cone)]);
    let output = success(inspect(root, "members", &overlays));
    let result: PackageMembersV1 = serde_json::from_str(&output).unwrap();
    result.validate().unwrap();
    assert_eq!(result.members.len(), 1);
    assert_eq!(result.members[0].coordinate.name(), "new");
    assert!(result.members[0].selected);
    let discovered = scoop_pm::Workspace::discover_with_overlays(
        root,
        Some(&member),
        scoop_pm::ManifestOverlays::new(overlays).unwrap(),
    )
    .unwrap();
    assert_eq!(discovered.root, fs::canonicalize(root).unwrap());
    assert_eq!(discovered.members.len(), 1);
    let invalid = "# 未保存\nschema = broken\n";
    let error = prepare::failure(
        inspect(
            root,
            "members",
            &input(&[(&workspace, &owner), (&member, invalid)]),
        ),
        "TOML",
    );
    let scoop_protocol::DiagnosticOriginV1::HostPathSpan { path, span } = error.origin() else {
        panic!()
    };
    assert_eq!(
        path.to_path_buf().unwrap(),
        fs::canonicalize(member.parent().unwrap())
            .unwrap()
            .join(member.file_name().unwrap())
    );
    assert!(!invalid[span.start() as usize..span.end() as usize].is_empty());
    prepare::failure(
        inspect(
            root,
            "metadata",
            &input(&[(&workspace, &owner), (&member, &cone)]),
        ),
        "no locked resolution",
    );
    assert_eq!(fs::read_to_string(workspace).unwrap(), original);
    assert!(!member.exists());
    assert!(!root.join("Cone.lock").exists());
}
