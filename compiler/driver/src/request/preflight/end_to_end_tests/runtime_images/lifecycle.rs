use super::support::check_runs;
use super::*;

#[test]
fn incomplete_real_image_inputs_fail_before_any_eager_code() {
    let target = resolved_target().unwrap();
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let provider = build_fixture(
        sysroot.path(),
        &target,
        "empty-provider",
        "library",
        &[],
        &[],
    );
    let root = build_fixture(
        sysroot.path(),
        &target,
        "eager-failure",
        "executable",
        &[&provider],
        &[],
    );
    let closure = runtime::read(&target, &[&core, &provider, &root]);
    let library = runtime::build(&target, &sysroot.path().join("runtime"));
    let template = std::fs::read_to_string(
        crate::workspace_root().join("tests/fixtures/m23-runtime-images/runtime.c"),
    )
    .unwrap();
    let count = "sizeof fixture_images / sizeof *fixture_images";
    assert!(template.contains(count));
    for (name, count_value) in [("empty", "0"), ("missing", "2")] {
        let template = template.replace(count, count_value);
        let executable = runtime::link_program(
            &target,
            &closure,
            &library,
            &template,
            &sysroot.path().join(name),
        );
        check_runs(&executable, "", Some("scoop runtime metadata:"));
    }
}
