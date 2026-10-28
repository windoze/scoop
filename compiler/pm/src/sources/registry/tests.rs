use super::*;

mod server;
use server::Server;

fn client(cache: &PackageCache) -> RegistryClient<'_> {
    RegistryClient {
        cache,
        http: ureq::Agent::config_builder().proxy(None).build().into(),
    }
}

const MANIFEST: &str = "schema = 1\n[cone]\ngroup = 'org.example'\nname = 'json'\nversion = '1.4.3'\nkind = 'library'\n";

fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    let encoder = zstd::stream::Encoder::new(Vec::new(), 1).unwrap();
    let mut archive = tar::Builder::new(encoder);
    for (path, contents) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_entry_type(tar::EntryType::Regular);
        // Raw headers also let extraction tests represent invalid archive paths.
        header.as_mut_bytes()[..path.len()].copy_from_slice(path.as_bytes());
        header.set_cksum();
        archive.append(&header, *contents).unwrap();
    }
    archive.into_inner().unwrap().finish().unwrap()
}

fn publish(server: &Server, bytes: Vec<u8>) -> (ConeCoordinate, RegistryVersion, RegistryUrl) {
    let (coordinate, kind, dependencies, _) = published_manifest(MANIFEST).unwrap();
    let version = RegistryVersion {
        version: coordinate.version().into(),
        kind,
        dependencies,
        archive: "archives/json.tar.zst".into(),
        sha256: scoop_wire::sha256(&bytes).to_string(),
        yanked: false,
    };
    server.set("/config.json", br#"{"format":1}"#.to_vec());
    server.set(
        "/packages/org.example/json.json",
        serde_json::to_vec(&RegistryIndex {
            format: 1,
            group: coordinate.group().into(),
            name: coordinate.name().into(),
            versions: vec![version.clone()],
        })
        .unwrap(),
    );
    server.set("/archives/json.tar.zst", bytes);
    (coordinate, version, server.url.parse().unwrap())
}

#[test]
fn two_projects_share_one_download_and_offline_reads_do_not_query_metadata() {
    let server = Server::new();
    let (coordinate, version, registry) = publish(
        &server,
        archive(&[
            ("Cone.toml", MANIFEST.as_bytes()),
            ("src/lib.scoop", b"package org.example\n"),
        ]),
    );
    let directory = tempfile::tempdir().unwrap();
    let cache_path = directory.path().join("shared");
    let a = PackageCache::resolve(&directory.path().join("project-a"), Some(&cache_path)).unwrap();
    let b = PackageCache::resolve(&directory.path().join("project-b"), Some(&cache_path)).unwrap();
    let first = fetch_locked(
        &client(&a),
        &registry,
        &coordinate,
        &version.archive,
        &version.sha256,
        false,
    )
    .unwrap();
    assert!(first.root.starts_with(&cache_path));
    assert_eq!(
        fs::read_to_string(first.root.join("src/lib.scoop")).unwrap(),
        "package org.example\n"
    );
    let requests = server.total();
    let second = fetch_locked(
        &client(&b),
        &registry,
        &coordinate,
        &version.archive,
        &version.sha256,
        true,
    )
    .unwrap();
    assert_eq!(first.root, second.root);
    assert_eq!(first.document, second.document);
    assert_eq!(server.count("/archives/json.tar.zst"), 1);
    assert_eq!(server.total(), requests);
    assert!(!directory.path().join("project-a").exists());
}

#[test]
fn concurrent_consumers_publish_one_copy_and_different_registries_do_not_alias() {
    let first = Server::new();
    let second = Server::new();
    let bytes = archive(&[("Cone.toml", MANIFEST.as_bytes())]);
    let (coordinate, version, registry) = publish(&first, bytes.clone());
    let (_, _, other_registry) = publish(&second, bytes);
    let directory = tempfile::tempdir().unwrap();
    let cache =
        PackageCache::resolve(directory.path(), Some(&directory.path().join("cache"))).unwrap();
    let client = client(&cache);
    let (a, b) = std::thread::scope(|scope| {
        let left = scope.spawn(|| fetch(&client, &registry, &coordinate, &version, false).unwrap());
        let right =
            scope.spawn(|| fetch(&client, &registry, &coordinate, &version, false).unwrap());
        (left.join().unwrap(), right.join().unwrap())
    });
    assert_eq!(a.root, b.root);
    assert_eq!(first.count("/archives/json.tar.zst"), 1);
    let other = fetch(&client, &other_registry, &coordinate, &version, false).unwrap();
    assert_ne!(a.root, other.root);
    assert_eq!(second.count("/archives/json.tar.zst"), 1);
}

#[test]
fn an_index_supplies_candidates_without_downloading_sources_and_yanks_keep_locked_inputs() {
    let server = Server::new();
    let (coordinate, mut version, registry) =
        publish(&server, archive(&[("Cone.toml", MANIFEST.as_bytes())]));
    let directory = tempfile::tempdir().unwrap();
    let cache =
        PackageCache::resolve(directory.path(), Some(&directory.path().join("cache"))).unwrap();
    let client = client(&cache);
    let key = DependencyCoordinateKey::from_coordinate(&coordinate);
    let mut index = load_index(&client, &registry, &key, false, false).unwrap();
    assert_eq!(index.versions.len(), 1);
    assert_eq!(server.count("/archives/json.tar.zst"), 0);
    let package = fetch(&client, &registry, &coordinate, &version, false).unwrap();
    version.yanked = true;
    index.versions[0] = version.clone();
    server.set(
        "/packages/org.example/json.json",
        serde_json::to_vec(&index).unwrap(),
    );
    assert!(
        load_index(&client, &registry, &key, false, true)
            .unwrap()
            .versions[0]
            .yanked
    );
    assert_eq!(
        fetch_locked(
            &client,
            &registry,
            &coordinate,
            &version.archive,
            &version.sha256,
            true
        )
        .unwrap()
        .root,
        package.root
    );
    let mut changed = version;
    changed.sha256 = "ff".repeat(32);
    assert!(
        fetch(&client, &registry, &coordinate, &changed, false)
            .unwrap_err()
            .to_string()
            .contains("immutable")
    );
    assert_eq!(server.count("/archives/json.tar.zst"), 1);
}

#[test]
fn failed_downloads_bad_metadata_and_bad_archives_do_not_publish_sources() {
    let server = Server::new();
    let (coordinate, mut version, registry) =
        publish(&server, archive(&[("Cone.toml", MANIFEST.as_bytes())]));
    let directory = tempfile::tempdir().unwrap();
    let cache =
        PackageCache::resolve(directory.path(), Some(&directory.path().join("cache"))).unwrap();
    let client = client(&cache);
    version.sha256 = "ff".repeat(32);
    assert!(
        fetch(&client, &registry, &coordinate, &version, false)
            .unwrap_err()
            .to_string()
            .contains("SHA-256 mismatch")
    );
    for (path, expected) in [
        ("../escape", "escapes"),
        ("/absolute", "escapes"),
        ("Cone.toml", "duplicate"),
    ] {
        let (_, version, _) = publish(
            &server,
            archive(&[("Cone.toml", MANIFEST.as_bytes()), (path, b"bad")]),
        );
        let error = fetch(&client, &registry, &coordinate, &version, false)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
        assert!(
            !cache
                .registry(&registry)
                .join("src/org.example/json/1.4.3")
                .exists()
        );
    }
    let bad_manifest = MANIFEST.replace("1.4.3", "1.5.0");
    let (_, version, _) = publish(&server, archive(&[("Cone.toml", bad_manifest.as_bytes())]));
    assert!(
        fetch(&client, &registry, &coordinate, &version, false)
            .unwrap_err()
            .to_string()
            .contains("does not match")
    );
    assert!(
        !cache
            .registry(&registry)
            .join("cache/org.example/json/1.4.3.tar.zst")
            .exists()
    );
}

#[test]
fn offline_missing_metadata_and_archives_fail_without_network_requests() {
    let server = Server::new();
    let (coordinate, version, registry) =
        publish(&server, archive(&[("Cone.toml", MANIFEST.as_bytes())]));
    let directory = tempfile::tempdir().unwrap();
    let cache =
        PackageCache::resolve(directory.path(), Some(&directory.path().join("cache"))).unwrap();
    let client = client(&cache);
    let key = DependencyCoordinateKey::from_coordinate(&coordinate);
    assert!(
        load_index(&client, &registry, &key, true, false)
            .unwrap_err()
            .to_string()
            .contains("not cached")
    );
    assert!(
        fetch(&client, &registry, &coordinate, &version, true)
            .unwrap_err()
            .to_string()
            .contains("not cached")
    );
    assert_eq!(server.total(), 0);
}

#[test]
fn malformed_index_records_fail_before_any_archive_download() {
    let server = Server::new();
    let (coordinate, version, registry) =
        publish(&server, archive(&[("Cone.toml", MANIFEST.as_bytes())]));
    let directory = tempfile::tempdir().unwrap();
    let cache = PackageCache::resolve(directory.path(), Some(directory.path())).unwrap();
    let client = client(&cache);
    let key = DependencyCoordinateKey::from_coordinate(&coordinate);
    let original = RegistryIndex {
        format: 1,
        group: coordinate.group().into(),
        name: coordinate.name().into(),
        versions: vec![version.clone()],
    };
    let mut duplicate = original.clone();
    duplicate.versions.push(version);
    let mut missing_core = original.clone();
    missing_core.versions[0].dependencies.clear();
    let mut abbreviated_git = original.clone();
    abbreviated_git.versions[0]
        .dependencies
        .push(RegistryDependency {
            key: "org.example:other".parse().unwrap(),
            version: "1.0.0".parse().unwrap(),
            source: RegistrySource::Git {
                git: "https://example.org/other.git".into(),
                commit: "abc123".into(),
                subdir: ".".into(),
            },
        });
    for (index, expected) in [
        (duplicate, "duplicate registry version"),
        (missing_core, "missing the exact implicit core"),
        (abbreviated_git, "full commit"),
    ] {
        server.set(
            "/packages/org.example/json.json",
            serde_json::to_vec(&index).unwrap(),
        );
        let error = load_index(&client, &registry, &key, false, false).unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
    server.set(
        "/packages/org.example/json.json",
        serde_json::to_vec(&original).unwrap(),
    );
    assert_eq!(
        load_index(&client, &registry, &key, false, false).unwrap(),
        original
    );
    assert_eq!(server.count("/packages/org.example/json.json"), 4);
    assert_eq!(server.count("/archives/json.tar.zst"), 0);
}
