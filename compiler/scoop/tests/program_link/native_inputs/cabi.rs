use super::*;

#[test]
fn native_c_abi_rejects_ordinary_layout_and_managed_static_callbacks() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    for (fixture, token, diagnostic) in [
        (
            "bad-layout",
            "@Extern(lib = \"m23_cabi\", name = \"m23_invalid\")\nfun invalid(value: Payload): Int",
            "ordinary struct `Payload` has no stable C layout",
        ),
        ("bad-callback", "::increment", "@NoGC"),
    ] {
        super::source::reject_source(
            environment,
            directory.path(),
            &[],
            &format!("cabi/{fixture}.scoop"),
            token,
            diagnostic,
        );
    }
}

#[test]
fn packed_c_layout_round_trips_through_a_native_object() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "cabi",
        "executable",
        &native_fixture("cabi/standalone.scoop"),
        &[],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "cabi",
        &[],
        &[],
        "cabi/standalone",
    );
    compile_native(
        directory.path(),
        "m23_cabi",
        &native_fixture("cabi/native.c"),
        &[],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let (program, _) = link_native(environment, &root, &[], directory.path());
    for stress in [false, true] {
        assert_eq!(run(&program, stress), "false\n42\n");
    }
}

#[test]
fn fixed_width_layout_pointer_and_static_callback_abis_survive_three_cones() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let provider = environment.build(
        directory.path(),
        "cabi-provider",
        "library",
        &native_fixture("cabi/provider.scoop"),
        &[],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "cabi-provider",
        &[],
        &[],
        "cabi/provider",
    );
    std::fs::remove_dir_all(directory.path().join("sources/cabi-provider")).unwrap();
    let facade = environment.build(
        directory.path(),
        "cabi-facade",
        "library",
        &native_fixture("cabi/facade.scoop"),
        &[("cabi-provider", &provider)],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "cabi-facade",
        &[&provider],
        &[],
        "cabi/facade",
    );
    std::fs::remove_dir_all(directory.path().join("sources/cabi-facade")).unwrap();
    let root = environment.build_with_support(
        directory.path(),
        "cabi-root",
        "executable",
        &native_fixture("cabi/root.scoop"),
        &[("cabi-facade", &facade)],
        &[&provider],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "cabi-root",
        &[&facade],
        &[&provider],
        "cabi/root",
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    for kind in ["object", "archive", "dynamic"] {
        let case = directory.path().join(kind);
        match kind {
            "object" => {
                compile_native(&case, "m23_cabi", &native_fixture("cabi/native.c"), &[]);
            }
            "archive" => {
                let object =
                    compile_native(&case, "payload", &native_fixture("cabi/native.c"), &[]);
                archive_native(&case, "m23_cabi", &[&object]);
            }
            "dynamic" => {
                dylib(
                    &case,
                    "m23_cabi",
                    &native_fixture("cabi/native.c"),
                    "@rpath/libm23_cabi.dylib",
                    &[],
                );
            }
            _ => unreachable!(),
        }
        std::fs::remove_dir_all(case.join("sources")).unwrap();
        let (program, _) = link_native(environment, &root, &[&provider, &facade], &case);
        for stress in [false, true] {
            assert_eq!(
                run(&program, stress),
                concat!(
                    "-128\n-32768\n-2147483648\n-9223372036854775808\n",
                    "255\n65535\n4294967295\n18446744073709551615\n",
                    "-2\n-4\n-6\n-8\n2\n4\n6\n8\n",
                    "4\nfalse\n17\n103\n42\n85\n84\ncabi-alive\n"
                ),
                "{kind}"
            );
        }
    }
}
