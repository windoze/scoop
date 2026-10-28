use std::{fs, path::Path};

use super::prepare::failure;
use super::{prepare::*, support::*};

pub(super) fn records(root: &Path) -> Vec<(std::path::PathBuf, std::time::SystemTime)> {
    let mut records: Vec<_> = fs::read_dir(root.join("cache/records/prepared-v1"))
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.path(), entry.metadata().unwrap().modified().unwrap())
        })
        .collect();
    records.sort();
    records
}

#[test]
fn prepare_reuses_configuration_after_source_edits_but_observes_new_workspace() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    core(root);
    app(root, "app", "");
    write(&root.join("app/src/main.scoop"), "fun main() {}\n");
    let mut request = request(root, &root.join("app"));
    let first = prepared(root, &request);
    let before = records(root);
    assert_eq!(before.len(), 2);
    write(
        &root.join("app/src/main.scoop"),
        "fun main() { println(42) }\n",
    );
    request.locked = true;
    assert_eq!(prepared(root, &request), first);
    assert_eq!(records(root), before);
    write(
        &root.join("Cone.toml"),
        "schema=1\n[workspace]\nmembers=['app']\n[overrides]\n'scoop:scoop.core'={path='core'}\n",
    );
    failure(invoke(root, &request), "no locked resolution");
    request.locked = false;
    let second = prepared(root, &request);
    assert_eq!(
        second.workspace.to_path_buf().unwrap(),
        fs::canonicalize(root).unwrap()
    );
    assert_eq!(second.resolutions[0].root, "app");
    assert_ne!(records(root), before);
}

#[test]
fn newly_created_user_configuration_invalidates_preparation() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    core(root);
    app(root, "app", "");
    let home = root.join("home");
    let configuration = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/scoop/config.toml")
    } else if cfg!(windows) {
        home.join("AppData/scoop/config.toml")
    } else {
        home.join(".config/scoop/config.toml")
    };
    let request = request(root, &root.join("app"));
    let invoke = || {
        command(root, &request)
            .env("HOME", &home)
            .env("APPDATA", home.join("AppData"))
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .output()
            .unwrap()
    };
    success(invoke());
    let before = records(root);
    success(invoke());
    assert_eq!(records(root), before);
    write(&configuration, "invalid TOML [");
    failure(invoke(), "config.toml");
    assert_eq!(records(root), before);
}

#[test]
fn newly_available_search_root_artifacts_invalidate_preparation() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write(&root.join("app/Cone.toml"), &cone("app", "1.0.0"));
    let relative = "scoop/scoop.core/0.1.0/cone.slib";
    super::artifact::artifact(
        &root.join("one").join(relative),
        scoop_identity::ConeCoordinate::reserved_core(),
        vec![],
        1,
    );
    let mut request = request(root, &root.join("app"));
    request.artifact_roots = ["one", "two"]
        .into_iter()
        .map(|name| scoop_protocol::HostPathCarrier::from_path(&root.join(name)).unwrap())
        .collect();
    prepared(root, &request);
    let before = records(root);
    super::artifact::artifact(
        &root.join("two").join(relative),
        scoop_identity::ConeCoordinate::reserved_core(),
        vec![],
        2,
    );
    failure(invoke(root, &request), "ambiguous artifacts");
    assert_eq!(records(root), before);
}
