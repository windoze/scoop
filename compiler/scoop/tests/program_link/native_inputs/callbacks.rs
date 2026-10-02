use super::*;

#[test]
fn imported_callbacks_preserve_source_contract_diagnostics() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    for (fixture, token, diagnostic) in [
        (
            "bad-signature",
            "foreignCallback<(String, Ptr<Unit>) -> Int>({ value: String -> 0 }, 1L, ForeignCallbackMode.OneShot)",
            "signature is not C-FFI-safe",
        ),
        (
            "bad-context",
            "2L",
            "contextIndex` is outside the native signature",
        ),
        (
            "bad-context-type",
            "1L",
            "context parameter must be exactly `Ptr<Unit>`",
        ),
        (
            "bad-mode",
            "foreignCallback<(Int, Ptr<Unit>) -> Int>({ value: Int -> value }, 1L, mode)",
            "mode must be the constant `Reusable` or `OneShot`",
        ),
        (
            "bad-unsafe",
            "foreignCallback<(Int, Ptr<Unit>) -> Int>({ value: Int -> value }, 1L, ForeignCallbackMode.Reusable)",
            "requires an unsafe context",
        ),
        (
            "bad-missing-signature",
            "foreignCallback({ value: Int -> value }, 1L, ForeignCallbackMode.Reusable)",
            "requires one explicit function type",
        ),
    ] {
        super::source::reject_source(
            environment,
            directory.path(),
            &[],
            &format!("callbacks/{fixture}.scoop"),
            token,
            diagnostic,
        );
    }
}

#[test]
fn native_same_thread_reentry_preserves_live_callback_roots() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "callback-root",
        "executable",
        &native_fixture("callbacks/root.scoop"),
        &[],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "callback-root",
        &[],
        &[],
        "callbacks/root",
    );
    compile_native(
        directory.path(),
        "m23_callbacks",
        &native_fixture("callbacks/native.c"),
        &[],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let (program, _) = link_native(environment, &root, &[], directory.path());
    for stress in [false, true] {
        assert_eq!(
            run(&program, stress),
            "42\ntrue\n42\ntrue\n1\ncallback-alive\n"
        );
    }
}

#[test]
fn foreign_thread_callback_failure_keeps_managed_payload_and_detaches() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let provider = environment.build(
        directory.path(),
        "callback-provider",
        "library",
        &native_fixture("callbacks/provider.scoop"),
        &[],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "callback-provider",
        &[],
        &[],
        "callbacks/provider",
    );
    std::fs::remove_dir_all(directory.path().join("sources/callback-provider")).unwrap();
    let root = environment.build(
        directory.path(),
        "callback-exception",
        "executable",
        &native_fixture("callbacks/exception.scoop"),
        &[("callback-provider", &provider)],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "callback-exception",
        &[&provider],
        &[],
        "callbacks/exception",
    );
    compile_native(
        directory.path(),
        "m23_callbacks",
        &native_fixture("callbacks/native.c"),
        &[],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let (program, _) = link_native(environment, &root, &[&provider], directory.path());
    for stress in [false, true] {
        assert_eq!(
            run(&program, stress),
            "0\ntrue\nfailure-7\n1\ncaller-alive\n"
        );
    }
}

#[test]
fn two_native_threads_keep_tls_independent_across_all_inputs_and_moving_gc() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let (root, provider, facade) =
        super::storage::build_storage_chain(environment, directory.path(), "callbacks/tls");
    for kind in ["object", "archive", "dynamic"] {
        let case = directory.path().join(kind);
        super::storage::prepare_storage_input(&case, kind);
        let (program, _) = link_native(environment, &root, &[&provider, &facade], &case);
        for stress in [false, true] {
            assert_eq!(
                run(&program, stress),
                "32\n32\n41\n32\n105\n81\ntrue\ntrue\n1\ncaller-alive\n",
                "{kind}"
            );
        }
    }
}
