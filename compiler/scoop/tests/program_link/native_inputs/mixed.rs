use super::*;

#[test]
fn four_cones_share_archive_dynamic_odr_and_initializers_after_source_removal() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let provider = environment.build(
        directory.path(),
        "mixed-provider",
        "library",
        &native_fixture("mixed/provider.scoop"),
        &[],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "mixed-provider",
        &[],
        &[],
        "mixed/provider",
    );
    std::fs::remove_dir_all(directory.path().join("sources/mixed-provider")).unwrap();
    let mut consumers = Vec::new();
    for name in ["left", "right"] {
        let cone = format!("mixed-{name}");
        consumers.push(environment.build(
            directory.path(),
            &cone,
            "library",
            &native_fixture(&format!("mixed/{name}.scoop")),
            &[("mixed-provider", &provider)],
        ));
        stage_snapshots(
            environment,
            directory.path(),
            &cone,
            &[&provider],
            &[],
            &format!("mixed/{name}"),
        );
        std::fs::remove_dir_all(directory.path().join("sources").join(&cone)).unwrap();
    }
    let root = environment.build(
        directory.path(),
        "mixed-root",
        "executable",
        &native_fixture("mixed/root.scoop"),
        &[
            ("mixed-provider", &provider),
            ("mixed-left", &consumers[0]),
            ("mixed-right", &consumers[1]),
        ],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "mixed-root",
        &[&provider, &consumers[0], &consumers[1]],
        &[],
        "mixed/root",
    );
    let members: Vec<_> = ["archive", "helper", "unused"]
        .iter()
        .map(|name| {
            compile_native(
                directory.path(),
                name,
                &native_fixture(&format!("mixed/{name}.c")),
                &[],
            )
        })
        .collect();
    archive_native(
        directory.path(),
        "m23_mix_archive",
        &members.iter().map(PathBuf::as_path).collect::<Vec<_>>(),
    );
    dylib(
        directory.path(),
        "m23_mix_dynamic",
        &native_fixture("mixed/dynamic.c"),
        "@rpath/libm23_mix_dynamic.dylib",
        &[],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let deps = [&provider as &Path, &consumers[0], &consumers[1]];
    let (program, plan) = link_native(environment, &root, &deps, directory.path());
    assert_eq!(plan.matches("selected=true").count(), 2, "{plan}");
    assert_eq!(plan.matches("selected=false").count(), 1, "{plan}");
    assert!(plan.contains("native library m23_mix_archive"), "{plan}");
    assert!(plan.contains("native library m23_mix_dynamic"), "{plan}");
    for stress in [false, true] {
        assert_eq!(
            run(&program, stress),
            "100\n0\n200\n56\n96\n121\nmixed-alive\n"
        );
    }
    let alias = directory.path().join("native-alias");
    std::os::unix::fs::symlink(directory.path().join("native"), &alias).unwrap();
    for roots in [["native", "native-alias"], ["native-alias", "native"]] {
        let output = checked(
            environment
                .link_command(&root, &[deps[2], deps[0], deps[1]], &program)
                .arg("--library-path")
                .arg(directory.path().join(roots[0]))
                .arg("--library-path")
                .arg(directory.path().join(roots[1])),
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), plan);
    }
}
