use super::*;

pub(super) fn check(
    target: &scoop_toolchain::ResolvedTargetProfile,
    root: &Path,
    workspace: &Path,
    sysroot: &Path,
    core: &Path,
    expected: &Path,
) {
    let Some(compiler) = std::env::var_os("SCOOP_TEST_PAIRED_SCOOPC") else {
        return;
    };
    let absent = workspace.join("cli-unused-sysroot");
    let run = |source: &Path, output: &Path, direct: &[&Path]| {
        let mut command = std::process::Command::new(&compiler);
        command
            .env("SCOOP_SYSROOT", &absent)
            .arg("build")
            .arg(source);
        for input in direct {
            command.arg("--direct-slib").arg(input);
        }
        let result = command.arg("--out-slib").arg(output).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    let output = workspace.join("cli-explicit.slib");
    run(root, &output, &[core]);
    assert_eq!(
        std::fs::read(&output).unwrap(),
        std::fs::read(expected).unwrap()
    );

    let helper = workspace.join("direct-helper");
    write_cone(&helper, "helper");
    let helper_artifact = workspace.join("direct-helper.slib");
    request(target, &helper, &helper_artifact, &absent, &[core], &[])
        .build_and_publish(DecodeLimits::default())
        .unwrap();
    let combined = workspace.join("direct-combined");
    write_cone(&combined, "combined");
    let combined_output = workspace.join("direct-combined.slib");
    run(&combined, &combined_output, &[&helper_artifact, core]);
    let expected_combined = workspace.join("direct-combined-api.slib");
    request(
        target,
        &combined,
        &expected_combined,
        &absent,
        &[core, &helper_artifact],
        &[],
    )
    .build_and_publish(DecodeLimits::default())
    .unwrap();
    assert_eq!(
        std::fs::read(combined_output).unwrap(),
        std::fs::read(&expected_combined).unwrap()
    );
    let implicit_combined = workspace.join("direct-combined-default.slib");
    request(
        target,
        &combined,
        &implicit_combined,
        sysroot,
        &[&helper_artifact],
        &[],
    )
    .build_and_publish(DecodeLimits::default())
    .unwrap();
    assert_eq!(
        std::fs::read(implicit_combined).unwrap(),
        std::fs::read(expected_combined).unwrap()
    );
    assert!(!absent.exists());
}
