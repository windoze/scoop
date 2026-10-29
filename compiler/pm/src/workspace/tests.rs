use super::*;
use scoop_manifest::package::PackageSource;

fn write(root: &Path, relative: &str, text: &str) {
    let file = root.join(relative).join("Cone.toml");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

fn cone(name: &str) -> String {
    format!(
        "schema = 1\n[cone]\ngroup = 'dev.example'\nname = '{name}'\nversion = '0.1.0'\nkind = 'library'\n"
    )
}

#[cfg(unix)]
#[test]
fn include_root_preserves_workspace_and_member_locators() {
    let directory = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(directory.path()).unwrap();
    write(
        &root,
        "real",
        "schema = 1\n[workspace]\nmembers = ['app']\n",
    );
    write(&root, "real/app", &cone("app"));
    std::os::unix::fs::symlink(root.join("real"), root.join("alias")).unwrap();
    let workspace = Workspace::discover(&root, Some(&root.join("alias"))).unwrap();
    let member = workspace.members.values().next().unwrap();
    assert_eq!(member.directory, root.join("real/app"));
    assert_eq!(member.input_directory, root.join("alias/app"));
    let workspace = Workspace::discover(&root, Some(&root.join("alias/app"))).unwrap();
    assert_eq!(
        workspace.members.values().next().unwrap().input_directory,
        root.join("alias/app")
    );
}

#[test]
fn expands_shared_fields_and_dependency_paths_at_the_definition() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".",
        "schema = 1\n[workspace]\nmembers = ['apps/server', 'libs/model']\ndefault-members = ['apps/server']\n[workspace.cone]\ngroup = 'dev.example'\nversion = '0.3.0'\n[workspace.dependencies]\n'dev.example:model' = { path = 'libs/model', version = '^0.3.0' }\n",
    );
    write(
        dir.path(),
        "apps/server",
        "schema = 1\n[cone]\ngroup = { workspace = true }\nname = 'server'\nversion = { workspace = true }\nkind = 'executable'\n[dependencies]\n'dev.example:model' = { workspace = true }\n",
    );
    write(
        dir.path(),
        "libs/model",
        &cone("model").replace("0.1.0", "0.3.1"),
    );
    let ws = Workspace::discover(dir.path(), None).unwrap();
    let selected = ws.select(&MemberSelection::Default).unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].cone.coordinate.version(), "0.3.0");
    let dep = &selected[0].cone.dependencies[&"dev.example:model".parse().unwrap()];
    assert_eq!(dep.base, ws.root);
    assert!(
        matches!(&dep.request.source, PackageSource::Path(path) if path == Path::new("libs/model"))
    );
    assert_eq!(dep.origin.manifest, ws.manifest);
    assert_eq!(
        dep.inherited_at.as_ref().unwrap().manifest,
        selected[0].manifest
    );
    assert_eq!(ws.select(&MemberSelection::All).unwrap().len(), 2);
}

#[test]
fn chooses_the_invoking_member_from_its_source_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".",
        "schema = 1\n[workspace]\nmembers = ['one', 'two']\ndefault-members = ['one']\n",
    );
    for name in ["one", "two"] {
        write(dir.path(), name, &cone(name));
    }
    std::fs::create_dir(dir.path().join("two/src")).unwrap();
    let ws = Workspace::discover(&dir.path().join("two/src"), None).unwrap();
    assert_eq!(
        ws.select(&MemberSelection::Default).unwrap()[0]
            .cone
            .coordinate
            .name(),
        "two"
    );
    assert_eq!(
        ws.select(&MemberSelection::Packages(vec![
            "dev.example:one".parse().unwrap()
        ]))
        .unwrap()[0]
            .cone
            .coordinate
            .name(),
        "one"
    );
    assert!(
        ws.select(&MemberSelection::Packages(vec![
            "dev.example:missing".parse().unwrap()
        ]))
        .is_err()
    );
}

#[test]
fn root_cone_is_implicit_and_unlisted_cones_are_independent() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".",
        &format!("{}[workspace]\nmembers = ['child']\n", cone("root")),
    );
    write(dir.path(), "child", &cone("child"));
    write(dir.path(), "independent", &cone("other"));
    let ws = Workspace::discover(dir.path(), None).unwrap();
    assert_eq!(ws.members.len(), 2);
    assert_eq!(
        ws.select(&MemberSelection::Default).unwrap()[0].relative_path,
        "."
    );
    let independent =
        Workspace::discover(dir.path(), Some(Path::new("independent/Cone.toml"))).unwrap();
    assert_eq!(independent.members.len(), 1);
    assert_eq!(
        independent.select(&MemberSelection::Default).unwrap()[0]
            .cone
            .coordinate
            .name(),
        "other"
    );
}

#[test]
fn duplicate_coordinates_and_bad_default_members_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".",
        "schema = 1\n[workspace]\nmembers = ['one', 'two']\n",
    );
    for name in ["one", "two"] {
        write(dir.path(), name, &cone("same"));
    }
    assert!(
        Workspace::discover(dir.path(), None)
            .unwrap_err()
            .message
            .contains("duplicate workspace coordinate")
    );
    write(
        dir.path(),
        ".",
        "schema = 1\n[workspace]\nmembers = ['one']\ndefault-members = ['two']\n",
    );
    assert!(
        Workspace::discover(dir.path(), None)
            .unwrap_err()
            .message
            .contains("not a workspace member")
    );
}

#[test]
fn nested_workspaces_do_not_inherit_outer_configuration() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".",
        "schema = 1\n[workspace]\nmembers = []\n[workspace.cone]\ngroup = 'dev.outer'\n",
    );
    write(
        dir.path(),
        "nested",
        "schema = 1\n[workspace]\nmembers = ['child']\n",
    );
    write(
        dir.path(),
        "nested/child",
        &cone("child").replace("group = 'dev.example'", "group = { workspace = true }"),
    );
    let error = Workspace::discover(&dir.path().join("nested/child"), None).unwrap_err();
    assert!(error.message.contains("does not define cone.group"));
    write(
        dir.path(),
        ".",
        "schema = 1\n[workspace]\nmembers = ['nested']\n",
    );
    assert!(
        Workspace::discover(dir.path(), None)
            .unwrap_err()
            .message
            .contains("nested workspace")
    );
}

#[test]
fn inheritance_requires_membership_and_a_shared_dependency() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".",
        &format!(
            "{}[dependencies]\n'org.example:json' = {{ workspace = true }}\n",
            cone("standalone")
        ),
    );
    let error = Workspace::discover(dir.path(), None).unwrap_err();
    assert!(error.message.contains("not a workspace member"));
    assert_eq!(error.origins.len(), 1);
}

#[test]
fn invalid_inherited_values_report_definition_and_reference() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".",
        "schema = 1\n[workspace]\nmembers = ['child']\n[workspace.cone]\nversion = 'invalid'\n",
    );
    write(
        dir.path(),
        "child",
        &cone("child").replace("version = '0.1.0'", "version = { workspace = true }"),
    );
    let error = Workspace::discover(dir.path(), None).unwrap_err();
    assert_eq!(error.origins.len(), 2);
    assert!(error.origins[0].manifest.ends_with("Cone.toml"));
    assert!(error.origins[1].manifest.ends_with("child/Cone.toml"));
}

#[test]
fn an_unlisted_cone_does_not_load_missing_sibling_members() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".",
        "schema = 1\n[workspace]\nmembers = ['missing']\n",
    );
    write(dir.path(), "independent", &cone("other"));
    assert!(Workspace::discover(&dir.path().join("independent"), None).is_ok());
    assert!(Workspace::discover(dir.path(), None).is_err());
}
