use super::*;

mod archive;
mod members;

#[test]
fn artifact_graph_rejects_missing_stale_extra_and_conflicting_inputs() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let library_source = fixture("basic-library.scoop");
    let library = environment.build(path, "library", "library", &library_source, &[]);
    let root = environment.build(
        path,
        "root",
        "executable",
        &fixture("basic-root.scoop"),
        &[("library", &library)],
    );
    let changed = environment.build(
        &path.join("changed"),
        "library",
        "library",
        &library_source.replace("40", "41"),
        &[],
    );
    let extra = environment.build(path, "extra", "library", &library_source, &[]);
    let executable = environment.build(path, "other", "executable", &fixture("empty.scoop"), &[]);
    let unresolved = environment.build(
        &path.join("unresolved"),
        "root",
        "executable",
        &format!(
            "{}\n@Extern(name = \"m23_missing_native\")\nprivate fun missing(): Int\n",
            fixture("basic-root.scoop")
        ),
        &[("library", &library)],
    );
    let version_source = path.join("version-source");
    copy_tree(&path.join("sources/library"), &version_source);
    let manifest = version_source.join("Cone.toml");
    let text = std::fs::read_to_string(&manifest)
        .unwrap()
        .replace("0.1.0", "0.2.0");
    std::fs::write(manifest, text).unwrap();
    let version = path.join("version.slib");
    checked(
        Command::new(&environment.compiler)
            .arg("build")
            .arg(&version_source)
            .arg("--direct-slib")
            .arg(&environment.core)
            .arg("--out-slib")
            .arg(&version),
    );
    for source in [
        path.join("sources"),
        path.join("changed/sources"),
        path.join("unresolved/sources"),
        version_source,
    ] {
        std::fs::remove_dir_all(source).unwrap();
    }

    let output = path.join("program");
    environment.link(&root, &[&library], &output);
    for (input, dependencies, expected) in [
        (&library, vec![], "executable Cone"),
        (&root, vec![], "MissingDirectArtifact"),
        (&root, vec![&changed], "StaleDependency"),
        (
            &root,
            vec![&library, &changed],
            "conflicting artifact contents",
        ),
        (&root, vec![&library, &extra], "UnreachableSupport"),
        (&root, vec![&library, &executable], "InvalidProviderKind"),
        (&root, vec![&library, &version], "MultipleVersions"),
        (&unresolved, vec![&changed], "StaleDependency"),
    ] {
        let dependencies = dependencies
            .iter()
            .map(|path| path.as_path())
            .collect::<Vec<_>>();
        assert_error(environment, input, &dependencies, &output, expected);
    }
    let expected_origin =
        scoop_identity::ConeCoordinate::new("dev.programlink", "library", "0.1.0")
            .unwrap()
            .identity()
            .unwrap()
            .to_string();
    let error = assert_error(environment, &root, &[&changed], &output, "StaleDependency");
    assert!(error.contains(&expected_origin), "{error}");
    assert_eq!(run(&output, false), "42\n");
}

fn assert_error(
    environment: &Environment,
    root: &Path,
    dependencies: &[&Path],
    output: &Path,
    expected: &str,
) -> String {
    let previous = std::fs::read(output).unwrap();
    let result = environment
        .link_command(root, dependencies, output)
        .output()
        .unwrap();
    assert!(!result.status.success(), "expected {expected}");
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(stderr.contains(expected), "expected {expected}: {stderr}");
    assert_eq!(std::fs::read(output).unwrap(), previous);
    stderr
}
