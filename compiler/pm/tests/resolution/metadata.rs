use std::{fs, path::Path};

use scoop_pm::cache::PackageCache;
use scoop_protocol::package::{PreparedConeV1, PreparedWorkspaceV1};

use super::{git, prepare, registry, server::Server, support::*};

fn metadata(root: &Path, app: &Path) -> PreparedWorkspaceV1 {
    let sysroot = root.join("sysroot");
    let output = success(pm(
        root,
        app,
        &[
            "metadata",
            "--offline",
            "--json",
            "--sysroot",
            sysroot.to_str().unwrap(),
        ],
    ));
    let result: PreparedWorkspaceV1 = serde_json::from_str(&output).unwrap();
    result.validate().unwrap();
    result
}

fn source(result: &PreparedWorkspaceV1, name: &str) -> std::path::PathBuf {
    let node = result.resolutions[0]
        .nodes
        .iter()
        .find(|node| node.cone.coordinate().name() == name)
        .unwrap();
    let PreparedConeV1::Source { config } = &node.cone else {
        panic!("expected a source package")
    };
    config.root.to_path_buf().unwrap()
}

#[test]
fn metadata_requires_a_current_lock_and_reuses_the_prepared_model_without_writing_it() {
    let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = directory.path();
    core(root);
    app(
        root,
        "app",
        "'org.example:lib'={path='../lib', version='^1.0.0'}",
    );
    write(&root.join("lib/Cone.toml"), &cone("lib", "1.2.0"));
    let app = root.join("app");
    let lock = app.join("Cone.lock");
    prepare::failure(
        pm(root, &app, &["metadata", "--offline", "--json"]),
        "no locked resolution",
    );
    assert!(!lock.exists());
    let mut request = prepare::request(root, &app);
    request.target = scoop_toolchain::host_target_triple().unwrap().into();
    request.cwd = scoop_protocol::HostPathCarrier::from_path(&app).unwrap();
    request.input = None;
    let prepared = prepare::prepared(root, &request);
    let before = fs::metadata(&lock).unwrap().modified().unwrap();
    let records = super::prepare_cache::records(root);
    assert_eq!(metadata(root, &app), prepared);
    assert_eq!(metadata(root, &app), prepared);
    assert_eq!(fs::metadata(&lock).unwrap().modified().unwrap(), before);
    assert_eq!(super::prepare_cache::records(root), records);
    let original = fs::read(&lock).unwrap();
    write(&root.join("lib/Cone.toml"), &cone("lib", "2.0.0"));
    prepare::failure(
        pm(root, &app, &["metadata", "--offline", "--json"]),
        "dependency resolution failed",
    );
    assert_eq!(fs::read(lock).unwrap(), original);
}

#[test]
fn offline_metadata_does_not_checkout_available_git_objects() {
    let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = directory.path();
    core(root);
    let repo = root.join("repo");
    git::init(&repo);
    write(&repo.join("Cone.toml"), &cone("lib", "1.0.0"));
    let commit = git::commit(&repo);
    let url = url::Url::from_directory_path(&repo).unwrap();
    app(
        root,
        "app",
        &format!("'org.example:lib'={{git='{url}',rev='{commit}'}}"),
    );
    let app = root.join("app");
    success(pm(root, &app, &["resolve"]));
    let original_source = source(&metadata(root, &app), "lib");
    let why: serde_json::Value = serde_json::from_str(&success(pm(
        root,
        &app,
        &["deps", "--why", "org.example:lib", "--json"],
    )))
    .unwrap();
    assert_eq!(
        why["resolutions"][0]["source"]["references"],
        serde_json::json!([format!("rev:{commit}")])
    );
    let before = fs::read(app.join("Cone.lock")).unwrap();
    let isolated = root.join("objects-only");
    let cache = PackageCache::resolve(root, Some(&isolated.join("cache"))).unwrap();
    let db = cache.git(url.as_str()).join("db");
    fs::create_dir_all(db.parent().unwrap()).unwrap();
    git::git(
        root,
        &[
            "clone",
            "--mirror",
            repo.to_str().unwrap(),
            db.to_str().unwrap(),
        ],
    );
    let checkout = db.parent().unwrap().join("checkouts").join(&commit);
    prepare::failure(
        pm(&isolated, &app, &["metadata", "--offline", "--json"]),
        "run scoop pm fetch",
    );
    assert!(!checkout.exists());
    failure(
        pm(
            &isolated,
            &app,
            &["deps", "--why", "org.example:lib", "--json"],
        ),
        "run scoop pm fetch",
    );
    assert!(!checkout.exists());
    assert!(original_source.join("Cone.toml").exists());
    success(pm(&isolated, &app, &["fetch", "--offline"]));
    assert_eq!(
        source(&metadata(&isolated, &app), "lib"),
        fs::canonicalize(checkout).unwrap()
    );
    assert_eq!(fs::read(app.join("Cone.lock")).unwrap(), before);
}

#[test]
fn offline_metadata_does_not_extract_cached_registry_archives() {
    let server = Server::new();
    let version = registry::publish(&server, "lib", "1.0.0", "");
    registry::index(&server, "lib", vec![version]);
    let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = directory.path();
    core(root);
    app(
        root,
        "app",
        &format!(
            "'org.example:lib'={{version='1.0.0',registry='{}'}}",
            server.url
        ),
    );
    let app = root.join("app");
    success(pm(root, &app, &["resolve"]));
    let original_source = source(&metadata(root, &app), "lib");
    let before = fs::read(app.join("Cone.lock")).unwrap();
    let requests = server.total();
    let registry = server
        .url
        .parse::<scoop_manifest::package::RegistryUrl>()
        .unwrap();
    let original = PackageCache::resolve(root, Some(&root.join("cache")))
        .unwrap()
        .registry(&registry);
    let isolated = root.join("archives-only");
    let local = PackageCache::resolve(root, Some(&isolated.join("cache")))
        .unwrap()
        .registry(&registry);
    for relative in [
        "index/org.example/lib.json",
        "cache/org.example/lib/1.0.0.json",
        "cache/org.example/lib/1.0.0.tar.zst",
    ] {
        let path = local.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::copy(original.join(relative), path).unwrap();
    }
    prepare::failure(
        pm(&isolated, &app, &["metadata", "--offline", "--json"]),
        "run scoop pm fetch",
    );
    assert!(!local.join("src").exists());
    failure(
        pm(
            &isolated,
            &app,
            &["deps", "--why", "org.example:lib", "--json"],
        ),
        "run scoop pm fetch",
    );
    assert!(!local.join("src").exists());
    assert!(original_source.join("Cone.toml").exists());
    success(pm(&isolated, &app, &["fetch", "--offline"]));
    assert_eq!(
        source(&metadata(&isolated, &app), "lib"),
        local.join("src/org.example/lib/1.0.0")
    );
    assert_eq!(fs::read(app.join("Cone.lock")).unwrap(), before);
    assert_eq!(server.total(), requests);
    let empty = root.join("empty-cache");
    prepare::failure(
        pm(&empty, &app, &["metadata", "--offline", "--json"]),
        "run scoop pm fetch",
    );
    assert_eq!(server.total(), requests);
    let online = success(pm(&empty, &app, &["metadata", "--json"]));
    let online: PreparedWorkspaceV1 = serde_json::from_str(&online).unwrap();
    online.validate().unwrap();
    assert!(source(&online, "lib").starts_with(empty.join("cache")));
    assert_eq!(server.count("/archives/lib-1.0.0.tar.zst"), 2);
    assert_eq!(fs::read(app.join("Cone.lock")).unwrap(), before);
}
