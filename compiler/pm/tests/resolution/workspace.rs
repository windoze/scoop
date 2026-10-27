use std::fs;

use super::support::*;

#[test]
fn multiple_roots_are_independent_and_production_does_not_load_test_dependencies() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    core(root);
    write(
        &root.join("Cone.toml"),
        "schema = 1\n[workspace]\nmembers = ['server', 'cli']\n[workspace.cone]\ngroup = 'org.example'\nversion = '0.1.0'\n[overrides]\n'scoop:scoop.core' = { path = 'core' }\n",
    );
    for (name, version, library) in [("server", "1.0.0", "json1"), ("cli", "2.0.0", "json2")] {
        write(
            &root.join(name).join("Cone.toml"),
            &format!(
                "schema = 1\n[cone]\ngroup = {{workspace=true}}\nname = '{name}'\nversion = {{workspace=true}}\nkind = 'executable'\n[dependencies]\n'org.example:json' = {{path='../{library}', version='{version}'}}\n"
            ),
        );
        write(
            &root.join(library).join("Cone.toml"),
            &cone("json", version),
        );
    }
    success(pm(root, root, &["resolve", "--workspace"]));
    let locked = lock(root);
    assert_eq!(locked.resolutions.len(), 2);
    let selected = |name: &str| {
        locked
            .resolutions
            .iter()
            .find(|graph| graph.root == name)
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.coordinate.name() == "json")
            .unwrap()
            .coordinate
            .version()
    };
    assert_eq!(selected("server"), "1.0.0");
    assert_eq!(selected("cli"), "2.0.0");
    let bytes = fs::read(root.join("Cone.lock")).unwrap();
    let modified = fs::metadata(root.join("Cone.lock"))
        .unwrap()
        .modified()
        .unwrap();
    let path = root.join("server/Cone.toml");
    let mut manifest = fs::read_to_string(&path).unwrap();
    manifest
        .push_str("\n[test-dependencies]\n'org.example:testing' = {path='../missing-testing'}\n");
    write(&path, &manifest);
    success(pm(root, &root.join("server"), &["resolve", "--locked"]));
    assert_eq!(
        fs::metadata(root.join("Cone.lock"))
            .unwrap()
            .modified()
            .unwrap(),
        modified
    );
    failure(
        pm(
            root,
            root,
            &["resolve", "-p", "org.example:server", "--purpose", "test"],
        ),
        "missing-testing",
    );
    assert_eq!(fs::read(root.join("Cone.lock")).unwrap(), bytes);
    write(&root.join("json1/Cone.toml"), &cone("json", "3.0.0"));
    failure(
        pm(root, root, &["resolve", "--workspace"]),
        "requires 1.0.0",
    );
    assert_eq!(fs::read(root.join("Cone.lock")).unwrap(), bytes);
}

#[test]
fn root_overrides_replace_sources_but_keep_original_version_constraints() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    core(root);
    app(
        root,
        "app",
        "'org.example:json' = {version='1.0.0', git='https://invalid.example.org/unused.git'}",
    );
    let path = root.join("app/Cone.toml");
    let manifest = format!(
        "{}'org.example:json' = {{path='../fixed'}}\n",
        fs::read_to_string(&path).unwrap()
    );
    write(&path, &manifest);
    write(&root.join("fixed/Cone.toml"), &cone("json", "1.1.0"));
    success(pm(root, &root.join("app"), &["resolve", "--offline"]));
    let bytes = fs::read(root.join("app/Cone.lock")).unwrap();
    write(&root.join("fixed/Cone.toml"), &cone("json", "2.0.0"));
    failure(
        pm(root, &root.join("app"), &["resolve", "--offline"]),
        "requires 1.0.0",
    );
    assert_eq!(fs::read(root.join("app/Cone.lock")).unwrap(), bytes);
}
