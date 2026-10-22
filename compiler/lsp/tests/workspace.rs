mod support;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use support::Client;

fn uri(path: &Path) -> String {
    url::Url::from_file_path(path).unwrap().to_string()
}

fn write(root: &Path, name: &str, text: &str) -> PathBuf {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

fn setup(root: &Path) -> String {
    write(
        root,
        "core/Cone.toml",
        "schema = 1\n[cone]\ngroup = 'scoop'\nname = 'scoop.core'\nversion = '0.1.0'\nkind = 'library'\n",
    );
    write(
        root,
        "core/src/core.scoop",
        "package scoop.core\npublic class Marker\n",
    );
    let manifest = "schema = 1\n[cone]\ngroup = 'test'\nname = 'editor'\nversion = '1.0.0'\nkind = 'library'\n[dependencies]\n'scoop:scoop.core' = {version = '0.1.0', path = '../core'}\n";
    write(root, "app/Cone.toml", manifest);
    write(
        root,
        "app/src/value.scoop",
        "package test.editor\nfun value(): Int = 42\n",
    );
    manifest.to_owned()
}

fn initialize(client: &mut Client, roots: &[PathBuf], options: Value) {
    let result = client.request("initialize", json!({
        "processId": null, "capabilities": {"textDocument": {"documentSymbol": {"hierarchicalDocumentSymbolSupport": true}}},
        "workspaceFolders": roots.iter().map(|path| json!({"uri": uri(path), "name": path.file_name().unwrap().to_string_lossy()})).collect::<Vec<_>>(),
        "initializationOptions": options,
    })).response_result.unwrap();
    assert_eq!(
        result["capabilities"]["workspace"]["workspaceFolders"]["supported"],
        true
    );
    client.notify("initialized", json!({}));
}

#[test]
fn manifest_buffers_are_versioned_and_configuration_errors_leave_syntax_available() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let original = setup(root);
    let manifest = root.join("app/Cone.toml");
    let manifest_uri = uri(&manifest);
    let source_uri = uri(&root.join("app/src/value.scoop"));
    let mut client = Client::start();
    initialize(&mut client, &[root.join("app")], json!({}));
    client.open(&source_uri, "fun stillVisible() { incomplete. }\n");
    assert!(
        !client
            .diagnostics(&source_uri, Some(1))
            .as_array()
            .unwrap()
            .is_empty()
    );
    client.open(&manifest_uri, "# 中文\nschema = broken\n");
    let errors = client.diagnostics(&manifest_uri, Some(1));
    assert_eq!(errors[0]["code"], "SCOOP_PROJECT");
    assert_eq!(errors[0]["range"]["start"]["line"], 1);
    assert_eq!(client.symbols(&source_uri)[0]["name"], "stillVisible");
    std::fs::write(&manifest, original.clone() + "# changed on disk\n").unwrap();
    client.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": manifest_uri, "type": 2}]}),
    );
    client.notify("textDocument/didChange", json!({"textDocument": {"uri": manifest_uri, "version": 2}, "contentChanges": [{"text": original}]}));
    assert!(
        client
            .diagnostics(&manifest_uri, Some(2))
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(client.symbols(&source_uri)[0]["name"], "stillVisible");
    std::fs::write(&manifest, "schema = invalid\n").unwrap();
    client.notify(
        "textDocument/didClose",
        json!({"textDocument": {"uri": manifest_uri}}),
    );
    assert!(
        client
            .diagnostics(&manifest_uri, None)
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        client.diagnostics(&manifest_uri, None)[0]["code"],
        "SCOOP_PROJECT"
    );
    client.finish();
}

#[test]
fn target_and_test_dependency_changes_refresh_configuration_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let mut manifest = setup(root);
    manifest.push_str("[test-dependencies]\n 'test:checks' = '1.0.0'\n");
    std::fs::write(root.join("app/Cone.toml"), manifest).unwrap();
    let manifest_uri = uri(&root.join("app/Cone.toml"));
    let source_uri = uri(&root.join("app/src/value.scoop"));
    let mut client = Client::start();
    initialize(
        &mut client,
        &[root.join("app")],
        json!({"purpose": "production"}),
    );
    client.open(&source_uri, "fun value() = 42\n");
    assert!(
        client
            .diagnostics(&source_uri, Some(1))
            .as_array()
            .unwrap()
            .is_empty()
    );
    client.notify(
        "workspace/didChangeConfiguration",
        json!({"settings": {"scoop": {"purpose": "test"}}}),
    );
    let errors = client.diagnostics(&manifest_uri, None);
    assert!(errors[0]["message"].as_str().unwrap().contains("checks"));
    client.notify(
        "workspace/didChangeConfiguration",
        json!({"settings": {"scoop": {"purpose": "production"}}}),
    );
    assert!(
        client
            .diagnostics(&manifest_uri, None)
            .as_array()
            .unwrap()
            .is_empty()
    );
    client.notify(
        "workspace/didChangeConfiguration",
        json!({"settings": {"scoop": {"target": "unknown-cpu"}}}),
    );
    assert!(
        client.diagnostics(&manifest_uri, None)[0]["message"]
            .as_str()
            .unwrap()
            .contains("unsupported target")
    );
    assert_eq!(client.symbols(&source_uri)[0]["name"], "value");
    client.finish();
}

#[test]
fn workspace_folders_are_independent_and_removal_clears_old_errors() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    setup(root);
    let other = root.join("other");
    write(root, "other/Cone.toml", "schema = invalid\n");
    write(root, "other/src/other.scoop", "fun other() = 1\n");
    let other_uri = uri(&other.join("Cone.toml"));
    let mut client = Client::start();
    initialize(&mut client, &[root.join("app"), other.clone()], json!({}));
    assert_eq!(
        client.diagnostics(&other_uri, None)[0]["code"],
        "SCOOP_PROJECT"
    );
    let source_uri = uri(&root.join("app/src/value.scoop"));
    assert_eq!(client.symbols(&source_uri)[0]["name"], "value");
    client.notify(
        "workspace/didChangeWorkspaceFolders",
        json!({"event": {"added": [], "removed": [{"uri": uri(&other), "name": "other"}]}}),
    );
    assert!(
        client
            .diagnostics(&other_uri, None)
            .as_array()
            .unwrap()
            .is_empty()
    );
    client.notify(
        "workspace/didChangeWorkspaceFolders",
        json!({"event": {"added": [{"uri": uri(&other), "name": "other"}], "removed": []}}),
    );
    assert_eq!(
        client.diagnostics(&other_uri, None)[0]["code"],
        "SCOOP_PROJECT"
    );
    client.finish();
}
