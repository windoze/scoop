use std::{collections::BTreeSet, path::Path, time::SystemTime};

use scoop_analysis::{AnalysisHost, ProjectConfiguration, WorkspaceLoader};
use scoop_protocol::CompilationPurpose;
use scoop_source::SourceText;

use super::support::{resolve, setup_without_lock, write};

fn contents_and_modified(path: &Path) -> (Vec<u8>, SystemTime) {
    (
        std::fs::read(path).unwrap(),
        path.metadata().unwrap().modified().unwrap(),
    )
}

fn workspace(root: &Path) -> ProjectConfiguration {
    let mut configuration = setup_without_lock(root);
    let manifest = std::fs::read_to_string(root.join("app/Cone.toml")).unwrap();
    write(
        root,
        "other/Cone.toml",
        &manifest.replace("name = 'app'", "name = 'other'"),
    );
    write(root, "other/src/other.scoop", "fun other() {}\n");
    write(
        root,
        "Cone.toml",
        "schema = 1\n[workspace]\nmembers = ['app', 'core', 'other']\ndefault-members = ['app']\n",
    );
    configuration.root = root.to_owned();
    configuration
}

#[test]
fn missing_lock_is_generated_and_reused_without_building() {
    let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = directory.path();
    let configuration = setup_without_lock(root);
    let lockfile = root.join("app/Cone.lock");
    assert!(!lockfile.exists());
    let mut host = AnalysisHost::default();
    let mut loader = WorkspaceLoader::default();
    let inputs = loader.load(
        &mut host,
        std::slice::from_ref(&configuration),
        &BTreeSet::new(),
        None,
    );
    assert!(inputs.diagnostics.is_empty(), "{:?}", inputs.diagnostics);
    assert_eq!(inputs.projects.len(), 1);
    assert!(inputs.projects[0].complete);
    assert_eq!(inputs.projects[0].cones.len(), 2);
    let before = contents_and_modified(&lockfile);
    let source = write(root, "app/src/value.scoop", "fun changed() {}\n");
    let inputs = loader.load(&mut host, &[configuration], &BTreeSet::new(), None);
    assert!(inputs.diagnostics.is_empty(), "{:?}", inputs.diagnostics);
    assert_eq!(contents_and_modified(&lockfile), before);
    assert_eq!(
        host.source(host.file_source(&source).unwrap())
            .unwrap()
            .text
            .as_str(),
        "fun changed() {}\n"
    );
    assert!(!root.join("app/target").exists());
    assert!(!root.join("core/Cone.lock").exists());
}

#[test]
fn initial_workspace_lock_covers_all_members_and_configurations() {
    let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = directory.path();
    let production = workspace(root);
    let mut testing = production.clone();
    testing.name = "linux-tests".into();
    testing.target = scoop_toolchain::select_target("x86_64-unknown-linux-gnu").unwrap();
    testing.purpose = CompilationPurpose::Test;
    let configurations = [production, testing];
    let mut host = AnalysisHost::default();
    let mut loader = WorkspaceLoader::default();
    for _ in 0..2 {
        let inputs = loader.load(&mut host, &configurations, &BTreeSet::new(), None);
        assert!(inputs.diagnostics.is_empty(), "{:?}", inputs.diagnostics);
        assert_eq!(inputs.projects.len(), 6);
        assert!(inputs.projects.iter().all(|project| project.complete));
        for name in ["app", "linux-tests"] {
            assert_eq!(
                inputs
                    .projects
                    .iter()
                    .filter(|project| project.configuration.name == name)
                    .count(),
                3
            );
        }
    }
    assert!(root.join("Cone.lock").exists());
    for member in ["app", "core", "other"] {
        assert!(!root.join(member).join("Cone.lock").exists());
    }
}

#[test]
fn opening_a_member_initializes_only_its_workspace_lock_record() {
    let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = directory.path();
    let mut configuration = workspace(root);
    configuration.root = root.join("other/Cone.toml");
    let mut host = AnalysisHost::default();
    let mut loader = WorkspaceLoader::default();
    let inputs = loader.load(
        &mut host,
        std::slice::from_ref(&configuration),
        &BTreeSet::new(),
        None,
    );
    assert!(inputs.diagnostics.is_empty(), "{:?}", inputs.diagnostics);
    assert_eq!(inputs.projects.len(), 1);
    assert_eq!(inputs.projects[0].configuration.root, root.join("other"));
    let before = contents_and_modified(&root.join("Cone.lock"));
    assert!(!root.join("other/Cone.lock").exists());

    configuration.root = root.to_owned();
    let inputs = loader.load(&mut host, &[configuration], &BTreeSet::new(), None);
    assert_eq!(inputs.projects.len(), 1);
    assert_eq!(inputs.unresolved_members.len(), 2);
    assert!(
        inputs
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message.contains("resolve"))
    );
    assert_eq!(contents_and_modified(&root.join("Cone.lock")), before);
}

#[test]
fn explicit_lockfile_controls_creation_and_reuse() {
    for existing_default in [false, true] {
        let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let root = directory.path();
        let mut configuration = setup_without_lock(root);
        let default_lockfile = root.join("app/Cone.lock");
        let original = existing_default.then(|| {
            resolve(&configuration);
            contents_and_modified(&default_lockfile)
        });
        let lockfile = root.join("locks/editor.lock");
        configuration.lockfile = Some(lockfile.clone());
        let mut host = AnalysisHost::default();
        let mut loader = WorkspaceLoader::default();
        let inputs = loader.load(
            &mut host,
            std::slice::from_ref(&configuration),
            &BTreeSet::new(),
            None,
        );
        assert!(inputs.diagnostics.is_empty(), "{:?}", inputs.diagnostics);
        assert_eq!(inputs.projects.len(), 1);
        let before = contents_and_modified(&lockfile);
        let inputs = loader.load(&mut host, &[configuration], &BTreeSet::new(), None);
        assert!(inputs.diagnostics.is_empty(), "{:?}", inputs.diagnostics);
        assert_eq!(contents_and_modified(&lockfile), before);
        assert_eq!(default_lockfile.exists(), existing_default);
        if let Some(original) = original {
            assert_eq!(contents_and_modified(&default_lockfile), original);
        }
    }
}

#[test]
fn failed_initial_resolution_preserves_local_sources_and_retries_after_fix() {
    let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = directory.path();
    let configuration = setup_without_lock(root);
    let manifest = root.join("app/Cone.toml");
    let original = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        original.clone() + "'test:absent' = {version = '1.0.0', path = '../absent'}\n",
    )
    .unwrap();
    let mut host = AnalysisHost::default();
    let mut loader = WorkspaceLoader::default();
    let inputs = loader.load(
        &mut host,
        std::slice::from_ref(&configuration),
        &BTreeSet::new(),
        None,
    );
    assert!(inputs.projects.is_empty());
    assert_eq!(inputs.unresolved_members.len(), 1);
    assert!(inputs.diagnostics[0].message.contains("absent"));
    assert!(
        host.file_source(&root.join("app/src/value.scoop"))
            .is_some()
    );
    assert!(!root.join("app/Cone.lock").exists());

    write(
        root,
        "absent/Cone.toml",
        &original.replace("name = 'app'", "name = 'absent'"),
    );
    write(root, "absent/src/value.scoop", "fun available() {}\n");
    let inputs = loader.load(&mut host, &[configuration], &BTreeSet::new(), None);
    assert!(inputs.diagnostics.is_empty(), "{:?}", inputs.diagnostics);
    assert_eq!(inputs.projects[0].cones.len(), 3);
    assert!(root.join("app/Cone.lock").exists());
}

#[test]
fn failed_workspace_member_does_not_block_other_initial_resolutions() {
    let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = directory.path();
    let configuration = workspace(root);
    let manifest = root.join("app/Cone.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        manifest,
        text + "'test:absent' = {version = '1.0.0', path = '../absent'}\n",
    )
    .unwrap();
    let mut host = AnalysisHost::default();
    let inputs =
        WorkspaceLoader::default().load(&mut host, &[configuration], &BTreeSet::new(), None);
    assert_eq!(inputs.projects.len(), 2);
    assert!(inputs.projects.iter().all(|project| project.complete));
    assert_eq!(inputs.unresolved_members.len(), 1);
    assert!(inputs.diagnostics[0].message.contains("absent"));
    assert!(root.join("Cone.lock").exists());
}

#[test]
fn initial_lock_uses_saved_manifests_and_overlays_remain_read_only() {
    let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = directory.path();
    let configuration = setup_without_lock(root);
    let manifest = root.join("app/Cone.toml");
    let original = std::fs::read_to_string(&manifest).unwrap();
    let mut host = AnalysisHost::default();
    host.set_file_text(
        manifest.clone(),
        SourceText::new(
            original.clone() + "'test:unsaved' = {version = '1.0.0', path = '../unsaved'}\n",
        ),
    );
    let mut loader = WorkspaceLoader::default();
    let inputs = loader.load(
        &mut host,
        std::slice::from_ref(&configuration),
        &BTreeSet::from([manifest.clone()]),
        None,
    );
    assert!(inputs.projects.is_empty());
    assert!(inputs.diagnostics[0].message.contains("unsaved"));
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), original);
    let lockfile = root.join("app/Cone.lock");
    let before = contents_and_modified(&lockfile);
    assert!(!String::from_utf8_lossy(&before.0).contains("unsaved"));
    let inputs = loader.load(&mut host, &[configuration], &BTreeSet::new(), None);
    assert!(inputs.diagnostics.is_empty(), "{:?}", inputs.diagnostics);
    assert!(inputs.projects[0].complete);
    assert_eq!(contents_and_modified(&lockfile), before);
}
