use std::{
    collections::BTreeSet,
    fs,
    sync::{Arc, Barrier},
};

use super::*;
use crate::cache::PackageCache;

fn resolution(root: &str, version: &str) -> LockedResolution {
    let app = ConeCoordinate::new("dev.example", "app", "0.1.0").unwrap();
    let lib = ConeCoordinate::new("org.example", "json", version).unwrap();
    let core = ConeCoordinate::reserved_core();
    LockedResolution {
        root: root.into(),
        purpose: Purpose::Production,
        target: "aarch64-apple-darwin".into(),
        root_coordinate: app.clone(),
        nodes: vec![
            LockedNode {
                coordinate: app,
                kind: RequestedConeKind::Executable,
                source: LockedSource::Path { path: root.into() },
                dependencies: vec![lib.clone(), core.clone()],
                test_dependencies: vec![],
            },
            LockedNode {
                coordinate: lib,
                kind: RequestedConeKind::Library,
                source: LockedSource::Registry {
                    registry: "https://packages.example.org".parse().unwrap(),
                    archive: format!("archives/org.example/json/{version}.tar.zst"),
                    sha256: "ab".repeat(32),
                },
                dependencies: vec![core.clone()],
                test_dependencies: vec![],
            },
            LockedNode {
                coordinate: core,
                kind: RequestedConeKind::Library,
                source: LockedSource::Sysroot {
                    name: "scoop.core".into(),
                },
                dependencies: vec![],
                test_dependencies: vec![],
            },
        ],
    }
}

#[test]
fn round_trip_keeps_exact_coordinates_and_multiple_independent_resolutions() {
    let lock = Lockfile {
        format: 1,
        resolutions: vec![
            resolution("server", "1.4.3"),
            resolution("cli", "2.0.0+metadata"),
        ],
    };
    lock.validate().unwrap();
    let text = toml::to_string_pretty(&lock).unwrap();
    assert!(text.contains("root-coordinate = \"dev.example:app:0.1.0\""));
    assert!(text.contains("2.0.0+metadata"));
    assert!(!text.contains("packages/registry"));
    assert_eq!(toml::from_str::<Lockfile>(&text).unwrap(), lock);
}

#[test]
fn rejects_incomplete_conflicting_cyclic_and_unreachable_graphs() {
    let check = |graph: LockedResolution, expected: &str| {
        let error = graph.validate().unwrap_err().to_string();
        assert!(error.contains(expected), "{error}");
    };
    let original = resolution(".", "1.4.3");
    let mut graph = original.clone();
    graph.nodes.remove(1);
    check(graph, "missing or duplicate");
    let mut graph = original.clone();
    graph.nodes.push(resolution(".", "2.0.0").nodes.remove(1));
    check(graph, "more than one node");
    let mut graph = original.clone();
    graph.nodes[1]
        .dependencies
        .push(graph.root_coordinate.clone());
    check(graph, "cycle");
    let mut graph = original.clone();
    graph.nodes[0].dependencies.remove(0);
    check(graph, "unreachable");
    let mut graph = original.clone();
    graph.nodes[1].kind = RequestedConeKind::Executable;
    check(graph, "must be a library");
    let mut graph = original.clone();
    let dependency = graph.nodes[1].coordinate.clone();
    graph.nodes[0].test_dependencies.push(dependency);
    check(graph, "only allowed on a Test root");
    let mut graph = original.clone();
    graph.nodes[1].dependencies.clear();
    check(graph, "implicit core");
    let mut graph = original;
    graph.root = "apps/../server".into();
    check(graph, "normalized");
}

#[test]
fn tests_have_an_independent_graph_and_only_the_root_has_test_edges() {
    let mut graph = resolution(".", "1.4.3");
    graph.purpose = Purpose::Test;
    let dependency = graph.nodes[0].dependencies.remove(0);
    graph.nodes[0].test_dependencies.push(dependency);
    graph.validate().unwrap();
    let mut lock = Lockfile {
        format: 1,
        resolutions: vec![graph.clone(), resolution(".", "2.0.0")],
    };
    lock.validate().unwrap();
    lock.resolutions.push(graph);
    assert!(
        lock.validate()
            .unwrap_err()
            .to_string()
            .contains("duplicate lock resolution")
    );
}

#[test]
fn source_records_reject_wrong_fields_and_malformed_download_inputs() {
    let text = toml::to_string_pretty(&resolution(".", "1.4.3")).unwrap();
    assert!(
        toml::from_str::<LockedResolution>(&text.replace(
            "kind = \"registry\"",
            "kind = \"registry\"\ncommit = \"unrelated\""
        ))
        .is_err()
    );
    let mut graph = resolution(".", "1.4.3");
    let LockedSource::Registry { sha256, .. } = &mut graph.nodes[1].source else {
        unreachable!()
    };
    *sha256 = "bad".into();
    assert!(
        graph
            .validate()
            .unwrap_err()
            .to_string()
            .contains("SHA-256")
    );
    for width in [40, 64] {
        graph.nodes[1].source = LockedSource::Git {
            git: "https://example.org/repo.git".into(),
            commit: "a".repeat(width),
            subdir: "cones/json".into(),
        };
        graph.validate().unwrap();
    }
}

#[test]
fn publication_preserves_other_roots_and_purposes_prunes_deleted_members_and_skips_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let cache =
        PackageCache::resolve(directory.path(), Some(&directory.path().join("cache"))).unwrap();
    let path = directory.path().join("Cone.lock");
    let members: BTreeSet<_> = ["server".into(), "cli".into()].into();
    LockTransaction::open(&path, &cache)
        .unwrap()
        .publish(
            vec![resolution("server", "1.4.3"), resolution("cli", "2.0.0")],
            &members,
        )
        .unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    LockTransaction::open(&path, &cache)
        .unwrap()
        .publish(vec![resolution("server", "1.4.3")], &members)
        .unwrap();
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    let mut test = resolution("server", "1.5.0");
    test.purpose = Purpose::Test;
    let lock = LockTransaction::open(&path, &cache)
        .unwrap()
        .publish(vec![test], &members)
        .unwrap();
    assert_eq!(lock.resolutions.len(), 3);
    let retained = LockTransaction::open(&path, &cache)
        .unwrap()
        .publish(vec![], &["server".into()].into())
        .unwrap();
    assert_eq!(retained.resolutions.len(), 2);
    let bytes = fs::read(&path).unwrap();
    let mut bad = resolution("server", "2.0.0");
    bad.nodes.clear();
    assert!(
        LockTransaction::open(&path, &cache)
            .unwrap()
            .publish(vec![bad], &members)
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn concurrent_updates_to_distinct_roots_do_not_overwrite_each_other() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Cone.lock");
    let cache =
        PackageCache::resolve(directory.path(), Some(&directory.path().join("cache"))).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    std::thread::scope(|scope| {
        for root in ["server", "cli"] {
            let (barrier, path, cache) = (barrier.clone(), &path, &cache);
            scope.spawn(move || {
                barrier.wait();
                LockTransaction::open(path, cache)
                    .unwrap()
                    .publish(
                        vec![resolution(root, "1.4.3")],
                        &["server".into(), "cli".into()].into(),
                    )
                    .unwrap();
            });
        }
    });
    assert_eq!(Lockfile::read(&path).unwrap().unwrap().resolutions.len(), 2);
}
