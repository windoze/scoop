use super::*;

#[test]
fn include_file_changes_refresh_root_and_source_dependency_without_source_edits() {
    let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = directory.path();
    let configuration = setup(root);
    let core_sources =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sysroot/src/scoop.core");
    std::os::unix::fs::symlink(core_sources.join("src"), root.join("core/src/ordinary")).unwrap();
    std::os::unix::fs::symlink(
        core_sources.join("platform/unix"),
        root.join("core/src/platform"),
    )
    .unwrap();
    let app = root.join("app/Cone.toml");
    let manifest = std::fs::read_to_string(&app).unwrap();
    std::fs::write(
        &app,
        manifest + "'test:lib' = {version = '1.0.0', path = '../lib'}\n",
    )
    .unwrap();
    write(
        root,
        "lib/Cone.toml",
        "schema=1\n[cone]\ngroup='test'\nname='lib'\nversion='1.0.0'\nkind='library'\n[dependencies]\n'scoop:scoop.core'={version='0.1.0',path='../core'}\n",
    );
    write(
        root,
        "lib/src/library.scoop",
        "package example\nimport scoop.compiler.includeStr\npublic fun content(): String = includeStr(\"data.txt\")\n",
    );
    write(
        root,
        "app/src/value.scoop",
        "import scoop.compiler.includeStr\nimport example.content\nfun value(): String = includeStr(\"data.txt\") + content()\n",
    );
    let local = write(root, "app/src/data.txt", "local");
    let dependency = write(root, "lib/src/data.txt", "dependency");
    resolve(&configuration);
    let mut host = AnalysisHost::default();
    let mut loader = WorkspaceLoader::default();
    let inputs = loader.load(&mut host, &[configuration], &BTreeSet::new(), None);
    assert!(inputs.diagnostics.is_empty(), "{:?}", inputs.diagnostics);
    host.set_workspace(inputs);
    let source = host.file_source(&root.join("app/src/value.scoop")).unwrap();
    let snapshot = host.snapshot();
    let mut session = scoop_analysis::SemanticSession::default();
    assert!(session.diagnostics(&snapshot, source).unwrap().is_empty());
    let roots = session
        .compiler_inputs(&snapshot)
        .iter()
        .flat_map(|input| input.resource_roots())
        .collect::<BTreeSet<_>>();
    assert!(roots.contains(&std::fs::canonicalize(root.join("app")).unwrap()));
    assert!(roots.contains(&std::fs::canonicalize(root.join("lib")).unwrap()));
    std::fs::write(&local, [0xff]).unwrap();
    let diagnostics = session.diagnostics(&snapshot, source).unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("valid UTF-8")),
        "{diagnostics:?}"
    );
    std::fs::write(&local, "recovered").unwrap();
    assert!(session.diagnostics(&snapshot, source).unwrap().is_empty());
    std::fs::remove_file(&local).unwrap();
    assert!(
        session
            .diagnostics(&snapshot, source)
            .unwrap()
            .iter()
            .any(|d| d.message.contains("cannot read include input"))
    );
    std::fs::write(&local, "created").unwrap();
    assert!(session.diagnostics(&snapshot, source).unwrap().is_empty());
    std::fs::write(&dependency, [0xff]).unwrap();
    assert!(matches!(session.diagnostics(&snapshot, source),
        Err(scoop_analysis::ProjectAnalysisError::SourceDependencyUnavailable { diagnostics, .. })
        if diagnostics.iter().any(|d| d.message.contains("valid UTF-8"))));
    std::fs::write(&dependency, "restored").unwrap();
    assert!(session.diagnostics(&snapshot, source).unwrap().is_empty());
    let version = host.source(source).unwrap().version;
    host.invalidate_compiler_inputs();
    assert!(snapshot.check_cancelled().is_err());
    assert_eq!(host.source(source).unwrap().version, version);
    assert!(
        session
            .diagnostics(&host.snapshot(), source)
            .unwrap()
            .is_empty()
    );
}
