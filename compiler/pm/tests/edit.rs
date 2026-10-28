use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use scoop_manifest::package::{DependencyDeclaration, PackageSource};

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn pm(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_scoop-pm"))
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn failure(output: Output, expected: &str) {
    assert!(!output.status.success());
    let message = String::from_utf8(output.stderr).unwrap();
    assert!(message.contains(expected), "{message}");
}

fn cone() -> &'static str {
    "# Project comment\nschema = 1\n\n[cone] # Identity\ngroup = 'org.example'\nname = 'app'\nversion = '1.0.0'\nkind = 'library'\n"
}

fn dependency(path: &Path, key: &str) -> scoop_manifest::package::DependencyRequest {
    let text = fs::read_to_string(path).unwrap();
    let document =
        scoop_manifest::package::parse_package_manifest(&text, scoop_toolchain::SOURCE_TARGETS)
            .unwrap();
    let DependencyDeclaration::Request(value) = &document.dependencies[&key.parse().unwrap()]
    else {
        panic!()
    };
    value.clone()
}

#[test]
fn edits_keep_comments_other_tables_and_lock_and_reject_invalid_changes() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let path = root.join("Cone.toml");
    let text = format!(
        "{}\n[dependencies] # Runtime deps\n# Keep this explanation\n'org.example:lib' = '1.0.0' # Dependency note\n'org.example:other' = {{ path = 'missing' }} # Other note\n\n[test-dependencies]\n'org.example:tests' = '1.0.0' # Test note\n",
        cone()
    );
    write(&path, &text);
    write(
        &root.join("Cone.lock"),
        "Existing lock must not be parsed or updated\n",
    );
    success(pm(root, &["add", "org.example:lib", "--version", "^2.0.0"]));
    let changed = fs::read_to_string(&path).unwrap();
    assert_eq!(
        changed,
        text.replace("'1.0.0' # Dependency note", "\"^2.0.0\" # Dependency note")
    );
    let stamp = fs::metadata(&path).unwrap().modified().unwrap();
    success(pm(root, &["add", "org.example:lib", "--version", "^2.0.0"]));
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), stamp);
    for (args, expected) in [
        (
            vec!["add", "org.example:lib", "--version", "latest"],
            "version",
        ),
        (
            vec![
                "add",
                "org.example:lib",
                "--artifact",
                "not-built.slib",
                "--version",
                "1.0.0",
            ],
            "exact =version",
        ),
        (
            vec![
                "add",
                "org.example:lib",
                "--branch",
                "main",
                "--version",
                "1.0.0",
            ],
            "require a Git source",
        ),
        (
            vec![
                "add",
                "org.example:lib",
                "--purpose",
                "test",
                "--version",
                "1.0.0",
            ],
            "already declared",
        ),
    ] {
        failure(pm(root, &args), expected);
        assert_eq!(fs::read_to_string(&path).unwrap(), changed);
    }
    success(pm(
        root,
        &["remove", "org.example:tests", "--purpose", "test"],
    ));
    let removed = fs::read_to_string(&path).unwrap();
    assert!(removed.contains("# Dependency note") && removed.contains("# Other note"));
    assert!(!removed.contains("org.example:tests"));
    failure(
        pm(root, &["remove", "org.example:tests", "--purpose", "test"]),
        "not declared",
    );
    assert_eq!(
        fs::read_to_string(root.join("Cone.lock")).unwrap(),
        "Existing lock must not be parsed or updated\n"
    );
}

#[test]
fn member_selection_rebases_paths_and_validates_workspace_inheritance_before_write() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write(
        &root.join("Cone.toml"),
        "schema=1\n[workspace]\nmembers=['one','two']\n[workspace.dependencies]\n'org.example:shared'={path='not-created',version='1.0.0'}\n",
    );
    for name in ["one", "two"] {
        write(
            &root.join(name).join("Cone.toml"),
            &cone().replace("name = 'app'", &format!("name = '{name}'")),
        );
    }
    let path = root.join("one/Cone.toml");
    let before = fs::read(&path).unwrap();
    failure(
        pm(root, &["add", "org.example:lib", "--version", "1.0.0"]),
        "exactly one selected Cone",
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    success(pm(
        root,
        &[
            "add",
            "org.example:lib",
            "-p",
            "org.example:one",
            "--path",
            "future/lib",
        ],
    ));
    assert_eq!(
        dependency(&path, "org.example:lib").source,
        PackageSource::Path("../future/lib".into())
    );
    success(pm(
        root,
        &[
            "add",
            "org.example:binary",
            "-p",
            "org.example:one",
            "--artifact",
            "output/lib.slib",
            "--version",
            "=1.2.3",
        ],
    ));
    assert_eq!(
        dependency(&path, "org.example:binary").source,
        PackageSource::Artifact("../output/lib.slib".into())
    );
    success(pm(
        &root.join("one"),
        &["add", "org.example:shared", "--inherit"],
    ));
    let inherited = fs::read_to_string(&path).unwrap();
    assert!(inherited.contains("workspace = true"));
    failure(
        pm(
            root,
            &[
                "add",
                "org.example:absent",
                "-p",
                "org.example:one",
                "--inherit",
            ],
        ),
        "does not define dependency",
    );
    failure(
        pm(
            root,
            &[
                "add",
                "org.example:shared",
                "-p",
                "org.example:one",
                "--inherit",
                "--version",
                "1.0.0",
            ],
        ),
        "entire dependency declaration",
    );
    assert_eq!(fs::read_to_string(path).unwrap(), inherited);
    assert_eq!(
        fs::read_to_string(root.join("two/Cone.toml")).unwrap(),
        cone().replace("name = 'app'", "name = 'two'")
    );
    assert!(!root.join("Cone.lock").exists());
}

#[test]
fn replacing_a_table_dependency_keeps_its_comment_and_drops_the_previous_source() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let path = root.join("Cone.toml");
    write(
        &path,
        &format!(
            "{}\n[dependencies.'org.example:lib'] # Keep dependency note\nversion = '1.0.0'\npath = 'unavailable'\n",
            cone()
        ),
    );
    success(pm(
        root,
        &[
            "add",
            "org.example:lib",
            "--github",
            "example/repo",
            "--branch",
            "main",
            "--subdir",
            "packages/lib",
            "--version",
            "^1.2.0",
        ],
    ));
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("# Keep dependency note"));
    let PackageSource::Git(source) = dependency(&path, "org.example:lib").source else {
        panic!()
    };
    assert_eq!(source.repository, "https://github.com/example/repo.git");
    assert_eq!(
        source.reference,
        scoop_manifest::package::GitReference::Branch("main".into())
    );
    assert_eq!(source.subdir.as_str(), "packages/lib");
    success(pm(
        root,
        &[
            "add",
            "org.example:lib",
            "--registry",
            "https://registry.example.invalid",
            "--version",
            "=2.0.0",
        ],
    ));
    let value = dependency(&path, "org.example:lib");
    assert!(matches!(value.source, PackageSource::Registry(_)));
    assert!(!fs::read_to_string(&path).unwrap().contains("github"));
    assert!(!root.join("Cone.lock").exists());
}
