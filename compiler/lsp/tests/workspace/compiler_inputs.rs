use super::*;

fn real_core(root: &Path) {
    let core = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sysroot/src/scoop.core/src");
    std::os::unix::fs::symlink(core, root.join("core/src/ordinary")).unwrap();
}

fn start(root: &Path) -> Client {
    let mut client = Client::start();
    client.request("initialize", json!({
        "processId": null,
        "capabilities": {"workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}}},
        "workspaceFolders": [{"uri": uri(&root.join("app")), "name": "app"}],
        "initializationOptions": {"packageCacheDir": root.join("pm-cache")}
    })).response_result.unwrap();
    client.notify("initialized", json!({}));
    client
}

fn watched(client: &Client, path: &Path) -> bool {
    client.registrations.values().any(|registration| {
        registration["registerOptions"]["watchers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|watcher| {
                glob::Pattern::new(watcher["globPattern"].as_str().unwrap())
                    .unwrap()
                    .matches_path(path)
            })
    })
}

fn notify(client: &mut Client, path: &Path, kind: u32) {
    assert!(
        watched(client, path),
        "resource is not watched: {}",
        path.display()
    );
    client.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": uri(path), "type": kind}]}),
    );
}

fn clean(client: &mut Client, source: &str, version: i32) {
    let diagnostics = client.diagnostics(source, Some(version));
    assert_eq!(diagnostics, json!([]), "{diagnostics}");
}

fn error(client: &mut Client, source: &str, text: &str) {
    let diagnostics = client.diagnostics(source, Some(1));
    assert!(
        diagnostics
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"].as_str().unwrap().contains(text)),
        "{diagnostics}"
    );
}

#[test]
fn file_resources_refresh_diagnostics_and_retire_unused_watches_without_source_edits() {
    let directory = tempfile::Builder::new()
        .prefix("scoop inputs ")
        .tempdir()
        .unwrap();
    let root = directory.path();
    setup(root);
    real_core(root);
    let text = "import scoop.compiler.includeStr\nimport scoop.compiler.includeBytes\nfun text(): String = includeStr(\"data.txt\")\nfun bytes(): Array<Byte> = includeBytes(\"data.bin\")\n";
    let source = uri(&write(root, "app/src/value.scoop", text));
    let binary = write(root, "app/src/data.bin", "bytes");
    let resource = root.join("app/src/data.txt");
    let mut client = start(root);
    client.open(&source, text);
    error(&mut client, &source, "cannot read include input");
    assert!(watched(&client, &resource));
    std::fs::write(&resource, "created").unwrap();
    notify(&mut client, &resource, 1);
    clean(&mut client, &source, 1);
    std::fs::write(&resource, [0xff]).unwrap();
    notify(&mut client, &resource, 2);
    error(&mut client, &source, "valid UTF-8");
    std::fs::write(&resource, "restored").unwrap();
    notify(&mut client, &resource, 2);
    clean(&mut client, &source, 1);
    std::fs::remove_file(&binary).unwrap();
    notify(&mut client, &binary, 3);
    error(&mut client, &source, "cannot read include input");
    std::fs::write(&binary, [0, 0xff]).unwrap();
    notify(&mut client, &binary, 1);
    clean(&mut client, &source, 1);

    let unrelated = write(root, "app/generated/output.bin", "unrelated output");
    notify(&mut client, &unrelated, 1);
    client.symbols(&source);
    assert!(
        !client
            .notifications
            .iter()
            .any(
                |notification| notification.method == "textDocument/publishDiagnostics"
                    && notification.params["uri"] == source
            )
    );

    client.notify("textDocument/didChange", json!({"textDocument": {"uri": source, "version": 2}, "contentChanges": [{"text": "fun value(): Int = 42\n"}]}));
    clean(&mut client, &source, 2);
    assert!(!watched(&client, &resource));
    client.finish();
}

#[test]
fn directory_resources_in_source_dependencies_watch_nested_entries_and_link_targets() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let manifest = setup(root);
    real_core(root);
    std::fs::write(
        root.join("app/Cone.toml"),
        manifest + "'test:lib' = {version = '1.0.0', path = '../lib'}\n",
    )
    .unwrap();
    write(
        root,
        "lib/Cone.toml",
        "schema=1\n[cone]\ngroup='test'\nname='lib'\nversion='1.0.0'\nkind='library'\n[dependencies]\n'scoop:scoop.core'={version='0.1.0',path='../core'}\n",
    );
    let text = "package example\nimport scoop.compiler.includeDir\nimport scoop.compiler.EmbeddedDir\npublic fun files(): EmbeddedDir = includeDir(\"assets\", exclude = [\"skip/**\"])\n";
    let source = uri(&write(root, "lib/src/library.scoop", text));
    let app_text = "import example.files\nfun count(): Long = files().files.size\n";
    let app_source = uri(&write(root, "app/src/value.scoop", app_text));
    let included = write(root, "lib/src/assets/data.txt", "data");
    write(root, "lib/src/assets/skip/unused.txt", "excluded");
    let target = write(root, "lib/other/target.txt", "linked");
    std::os::unix::fs::symlink(root.join("lib/other"), root.join("lib/src/assets/linked")).unwrap();
    let output = std::process::Command::new(
        Path::new(env!("CARGO_BIN_EXE_scoop-lsp")).with_file_name("scoop-pm"),
    )
    .args(["resolve", "--offline", "--purpose", "production"])
    .arg(root.join("app"))
    .arg("--package-cache-dir")
    .arg(root.join("pm-cache"))
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut client = start(root);
    client.open(&app_source, app_text);
    clean(&mut client, &app_source, 1);
    assert!(watched(&client, &target));
    client.open(&source, text);
    clean(&mut client, &source, 1);

    let nested = write(root, "lib/src/assets/new/deep/file.bin", "new");
    notify(&mut client, &nested, 1);
    clean(&mut client, &source, 1);
    std::fs::remove_file(&included).unwrap();
    notify(&mut client, &included, 3);
    clean(&mut client, &source, 1);
    std::fs::write(&target, "changed target").unwrap();
    notify(&mut client, &target, 2);
    clean(&mut client, &source, 1);

    let outside = write(root, "outside.txt", "outside");
    let link = root.join("lib/src/assets/skip/outside");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    notify(&mut client, &link, 1);
    error(&mut client, &source, "outside the defining Cone");
    std::fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    notify(&mut client, &link, 2);
    clean(&mut client, &source, 1);
    client.finish();
}
