#[cfg(unix)]
#[path = "workspace/compiler_inputs.rs"]
mod compiler_inputs;
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
    for purpose in ["production", "test"] {
        let output = std::process::Command::new(
            std::path::Path::new(env!("CARGO_BIN_EXE_scoop-lsp"))
                .with_file_name(format!("scoop-pm{}", std::env::consts::EXE_SUFFIX)),
        )
        .args(["resolve", "--offline", "--purpose", purpose])
        .arg(root.join("app"))
        .arg("--package-cache-dir")
        .arg(root.join("pm-cache"))
        .output()
        .expect("build the companion scoop-pm before project integration tests");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
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

fn workspace_symbols(client: &mut Client, query: &str) -> Value {
    client
        .request("workspace/symbol", json!({"query": query}))
        .response_result
        .unwrap()
}

#[test]
fn workspace_symbols_follow_buffers_disk_and_project_membership() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    setup(root);
    let source = root.join("app/src/value.scoop");
    let source_uri = uri(&source);
    let test = write(root, "app/tests/check.scoop", "fun testOnly() {}\n");
    let mut client = Client::start();
    initialize(&mut client, &[root.join("app")], json!({}));
    let initial = workspace_symbols(&mut client, "VALUE");
    assert_eq!(initial.as_array().unwrap().len(), 1);
    assert_eq!(initial[0]["location"]["uri"], source_uri);
    assert_eq!(
        initial[0]["location"]["range"]["start"],
        json!({"line": 1, "character": 4})
    );
    assert!(
        workspace_symbols(&mut client, "testOnly")
            .as_array()
            .unwrap()
            .is_empty()
    );
    client.open(
        &source_uri,
        "class `😀名` { fun member() { fun hidden() {} } }\nfun broken(value: ) {}\n",
    );
    assert!(
        workspace_symbols(&mut client, "value")
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        workspace_symbols(&mut client, "hidden")
            .as_array()
            .unwrap()
            .is_empty()
    );
    let names = workspace_symbols(&mut client, "😀");
    assert_eq!(names[0]["name"], "😀名");
    assert_eq!(
        names[0]["location"]["range"],
        json!({"start": {"line": 0, "character": 6}, "end": {"line": 0, "character": 11}})
    );
    let members = workspace_symbols(&mut client, "member");
    assert_eq!(members[0]["containerName"], "😀名");
    assert_eq!(members[0]["kind"], 6);
    assert_eq!(
        workspace_symbols(&mut client, "broken")[0]["location"]["range"]["start"],
        json!({"line": 1, "character": 4})
    );
    std::fs::write(&source, "fun changedOnDisk() {}\n").unwrap();
    client.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": source_uri, "type": 2}]}),
    );
    assert_eq!(
        workspace_symbols(&mut client, "broken")
            .as_array()
            .unwrap()
            .len(),
        1
    );
    client.notify(
        "textDocument/didClose",
        json!({"textDocument": {"uri": source_uri}}),
    );
    assert!(
        workspace_symbols(&mut client, "broken")
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        workspace_symbols(&mut client, "changedOnDisk")
            .as_array()
            .unwrap()
            .len(),
        1
    );
    client.notify(
        "workspace/didChangeConfiguration",
        json!({"settings": {"scoop": {"purpose": "test"}}}),
    );
    assert_eq!(
        workspace_symbols(&mut client, "testOnly")[0]["location"]["uri"],
        uri(&test)
    );
    client.notify(
        "workspace/didChangeConfiguration",
        json!({"settings": {"scoop": {"purpose": "production"}}}),
    );
    assert!(
        workspace_symbols(&mut client, "testOnly")
            .as_array()
            .unwrap()
            .is_empty()
    );
    client.notify(
        "workspace/didChangeWorkspaceFolders",
        json!({"event": {
            "added": [], "removed": [{"uri": uri(&root.join("app")), "name": "app"}]
        }}),
    );
    assert!(
        workspace_symbols(&mut client, "")
            .as_array()
            .unwrap()
            .is_empty()
    );
    client.finish();
}
