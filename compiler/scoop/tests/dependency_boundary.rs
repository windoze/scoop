use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    resolve: Resolve,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    id: String,
    deps: Vec<NodeDependency>,
}

#[derive(Deserialize)]
struct NodeDependency {
    pkg: String,
    dep_kinds: Vec<DependencyKind>,
}

#[derive(Deserialize)]
struct DependencyKind {
    kind: Option<String>,
}

#[test]
fn orchestration_cannot_reach_compiler_implementation_crates() {
    let metadata = workspace_metadata();
    let reachable = normal_dependency_names(&metadata, "scoop");
    let forbidden = HashSet::from([
        "scoopc",
        "scoop-parser",
        "scoop-hir-lower",
        "scoop-mir-lower",
        "scoop-lir-lower",
        "scoop-codegen",
    ]);

    for name in reachable {
        assert!(
            !forbidden.contains(name),
            "scoop orchestration reaches forbidden implementation crate {name}"
        );
    }
}

#[test]
fn compiler_driver_cannot_reach_orchestration() {
    let metadata = workspace_metadata();
    assert!(
        !normal_dependency_names(&metadata, "scoopc").contains("scoop"),
        "scoopc must not depend on the scoop orchestration crate"
    );
}

fn workspace_metadata() -> Metadata {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .unwrap()
        .to_path_buf();
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1"])
        .current_dir(workspace)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn normal_dependency_names<'metadata>(
    metadata: &'metadata Metadata,
    root_name: &str,
) -> HashSet<&'metadata str> {
    let packages: HashMap<_, _> = metadata
        .packages
        .iter()
        .map(|package| (package.id.as_str(), package.name.as_str()))
        .collect();
    let nodes: HashMap<_, _> = metadata
        .resolve
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let root = metadata
        .packages
        .iter()
        .find(|package| package.name == root_name)
        .unwrap();
    let mut pending = VecDeque::from([root.id.as_str()]);
    let mut visited = HashSet::new();
    let mut names = HashSet::new();

    while let Some(package_id) = pending.pop_front() {
        if !visited.insert(package_id) {
            continue;
        }
        names.insert(packages[package_id]);
        for dependency in &nodes[package_id].deps {
            if dependency
                .dep_kinds
                .iter()
                .any(|kind| kind.kind.as_deref().is_none_or(|kind| kind == "normal"))
            {
                pending.push_back(dependency.pkg.as_str());
            }
        }
    }
    names
}
