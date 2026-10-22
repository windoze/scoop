use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use scoop_analysis::{AnalysisHost, ProjectConfiguration, WorkspaceLoader};
use scoop_protocol::CompilationPurpose;
use scoop_source::SourceText;

fn write(root: &Path, name: &str, text: &str) -> PathBuf {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

fn setup(root: &Path) -> ProjectConfiguration {
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
    write(
        root,
        "app/Cone.toml",
        "schema = 1\n[cone]\ngroup = 'test'\nname = 'app'\nversion = '1.0.0'\nkind = 'library'\n[dependencies]\n'scoop:scoop.core' = {version = '0.1.0', path = '../core'}\n",
    );
    write(
        root,
        "app/src/value.scoop",
        "package test.app\nfun value(): Int = 42\n",
    );
    ProjectConfiguration {
        name: "app".into(),
        root: root.join("app"),
        target: scoop_toolchain::select_target("aarch64-apple-darwin").unwrap(),
        purpose: CompilationPurpose::Production,
        sysroot: root.join("sysroot"),
        artifact_search_roots: Vec::new(),
    }
}

#[test]
fn opened_files_and_manifests_override_disk_and_close_restores_disk() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let configuration = setup(root);
    let path = root.join("app/src/value.scoop");
    let manifest = root.join("app/Cone.toml");
    let mut host = AnalysisHost::default();
    let mut loader = WorkspaceLoader::default();
    let inputs = loader.load(
        &mut host,
        std::slice::from_ref(&configuration),
        &BTreeSet::new(),
        None,
    );
    assert!(inputs.diagnostics.is_empty(), "{:?}", inputs.diagnostics);
    assert!(inputs.projects[0].complete);
    assert_eq!(inputs.projects[0].cones.len(), 2);
    host.set_workspace(inputs);
    let id = host.file_source(&path).unwrap();
    let original = host.source(id).unwrap().clone();
    original.parse().unwrap();
    let revision = host.revision();
    let unchanged = loader.load(
        &mut host,
        std::slice::from_ref(&configuration),
        &BTreeSet::new(),
        None,
    );
    host.set_workspace(unchanged);
    assert_eq!(host.revision(), revision);
    assert!(std::sync::Arc::ptr_eq(host.source(id).unwrap(), &original));

    host.set_file_text(
        path.clone(),
        SourceText::new("fun editing() { unknown. }".to_owned()),
    );
    let overlays = BTreeSet::from([path.clone()]);
    std::fs::write(&path, "fun onDisk(): Int = 123").unwrap();
    let inputs = loader.load(
        &mut host,
        std::slice::from_ref(&configuration),
        &overlays,
        None,
    );
    assert!(inputs.projects[0].complete);
    assert_eq!(
        host.source(id).unwrap().text.as_str(),
        "fun editing() { unknown. }"
    );

    host.set_file_text(
        manifest.clone(),
        SourceText::new("schema = broken".to_owned()),
    );
    let inputs = loader.load(
        &mut host,
        std::slice::from_ref(&configuration),
        &BTreeSet::from([path.clone(), manifest.clone()]),
        None,
    );
    assert_eq!(inputs.diagnostics.len(), 1);
    assert_eq!(inputs.diagnostics[0].path, manifest);
    assert!(inputs.diagnostics[0].span.is_some());
    assert!(
        !host
            .source(id)
            .unwrap()
            .parse()
            .unwrap()
            .diagnostics()
            .is_empty()
    );

    let restored = loader.load(&mut host, &[configuration], &BTreeSet::new(), None);
    assert!(restored.diagnostics.is_empty());
    assert_eq!(
        host.source(id).unwrap().text.as_str(),
        "fun onDisk(): Int = 123"
    );
    assert_eq!(host.file_source(&path), Some(id));
}

#[test]
fn targets_purposes_and_unsaved_files_share_text_but_keep_project_membership() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let mut production = setup(root);
    let manifest = root.join("app/Cone.toml");
    let mut text = std::fs::read_to_string(&manifest).unwrap();
    text = text.replace(
        "schema = 1",
        "schema = 1\nsources = [{path = 'src'}, {path = 'linux', when = {os = 'linux'}}]",
    );
    std::fs::write(manifest, text).unwrap();
    write(
        root,
        "app/linux/platform.scoop",
        "package test.app\nfun platform() = 1\n",
    );
    let test = write(
        root,
        "app/tests/check.scoop",
        "package test.app\nfun check() = 2\n",
    );
    production.name = "darwin".into();
    let mut testing = production.clone();
    testing.name = "linux-tests".into();
    testing.target = scoop_toolchain::select_target("x86_64-unknown-linux-gnu").unwrap();
    testing.purpose = CompilationPurpose::Test;
    let mut host = AnalysisHost::default();
    let extra = root.join("app/src/unsaved.scoop");
    let id = host.set_file_text(
        extra.clone(),
        SourceText::new("package test.app\nfun added() = 3\n".to_owned()),
    );
    let mut loader = WorkspaceLoader::default();
    let inputs = loader.load(
        &mut host,
        &[production, testing],
        &BTreeSet::from([extra]),
        Some("linux-tests".into()),
    );
    assert!(inputs.diagnostics.is_empty(), "{:?}", inputs.diagnostics);
    let first = inputs.projects[0].source_cone(id).unwrap().1;
    let second = inputs.projects[1].source_cone(id).unwrap().1;
    assert_eq!(first.sources.len(), 2);
    assert_eq!(second.sources.len(), 4);
    assert!(
        second
            .sources
            .iter()
            .any(|source| source.path == test && source.role == scoop_identity::SourceRole::Test)
    );
    assert_eq!(
        inputs.project_for_source(id).unwrap().configuration.name,
        "linux-tests"
    );
    assert_eq!(first.manifest.identity(), second.manifest.identity());
}

#[test]
fn configuration_failures_preserve_selected_source_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let configuration = setup(root);
    let manifest = root.join("app/Cone.toml");
    let source = root.join("app/src/value.scoop");
    let mut text = std::fs::read_to_string(&manifest).unwrap();
    text.push_str("'test:missing' = '1.0.0'\n");
    std::fs::write(&manifest, &text).unwrap();
    let mut host = AnalysisHost::default();
    let mut loader = WorkspaceLoader::default();
    let inputs = loader.load(
        &mut host,
        std::slice::from_ref(&configuration),
        &BTreeSet::new(),
        None,
    );
    assert!(!inputs.projects[0].complete);
    assert!(inputs.diagnostics[0].message.contains("not found"));
    assert!(host.file_source(&source).is_some());
    std::fs::write(
        &manifest,
        text.replace(
            "'test:missing' = '1.0.0'",
            "'test:app' = {version = '1.0.0', path = '.'}",
        ),
    )
    .unwrap();
    let inputs = loader.load(&mut host, &[configuration], &BTreeSet::new(), None);
    assert!(inputs.diagnostics[0].message.contains("cycle"));
    assert!(host.file_source(&source).is_some());
}

#[test]
fn semantic_inputs_keep_unavailable_source_dependencies_in_the_selected_graph() {
    let directory = tempfile::tempdir().unwrap();
    let configuration = setup(directory.path());
    let mut host = AnalysisHost::default();
    let mut loader = WorkspaceLoader::default();
    let inputs = loader.load(&mut host, &[configuration], &BTreeSet::new(), None);
    host.set_workspace(inputs);
    let source = host
        .file_source(&directory.path().join("app/src/value.scoop"))
        .unwrap();
    let mut session = scoop_analysis::SemanticSession::default();
    assert!(matches!(
        session.declarations(&host.snapshot(), source),
        Err(
            scoop_analysis::ProjectAnalysisError::SourceDependencyUnavailable(
                scoop_identity::ConeIdentity::CORE
            )
        )
    ));
    let old = host.snapshot();
    host.set_text(source, SourceText::new("fun changed(): Int = 1".to_owned()));
    assert!(matches!(
        session.declarations(&old, source),
        Err(scoop_analysis::ProjectAnalysisError::Cancelled)
    ));
    assert!(matches!(
        session.declarations(&host.snapshot(), source),
        Err(
            scoop_analysis::ProjectAnalysisError::SourceDependencyUnavailable(
                scoop_identity::ConeIdentity::CORE
            )
        )
    ));
}
