use super::*;

#[test]
fn real_gcc_override_belongs_only_to_musl_wrappers() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().as_os_str();
    let gcc = directory.path().join(crate::host_executable_name("gcc"));
    let selected = directory
        .path()
        .join(crate::host_executable_name("selected-gcc"));
    for file in [&gcc, &selected] {
        std::fs::write(file, "compiler locator").unwrap();
    }
    let selected = std::path::absolute(selected).unwrap();
    let missing = OsStr::new("missing-gcc");
    assert_eq!(
        underlying_compiler(LinuxLibc::Gnu, path, Some(missing)).unwrap(),
        None,
    );
    assert_eq!(
        underlying_compiler(LinuxLibc::Musl, path, None).unwrap(),
        Some(std::path::absolute(gcc).unwrap()),
    );
    assert_eq!(
        underlying_compiler(LinuxLibc::Musl, path, Some(selected.as_os_str())).unwrap(),
        Some(selected),
    );
    assert!(underlying_compiler(LinuxLibc::Musl, path, Some(missing)).is_err());
}

#[test]
fn probe_and_invocation_use_the_same_libc_specific_environment() {
    use scoop_lir::GccCompilerIdentityV1;
    for (target, real_gcc) in [
        (scoop_target_data::LINUX_X86_64_GNU, None),
        (
            scoop_target_data::LINUX_X86_64_MUSL,
            Some(std::path::absolute("/tools/gcc").unwrap()),
        ),
    ] {
        let paths = CompilerPaths {
            driver: std::path::absolute("/tools/selected-cc").unwrap(),
            real_gcc: real_gcc.clone(),
            search_path: OsString::from("/tools"),
            sysroot: None,
            temporary: PathBuf::from("/work/temporary"),
        };
        let compiler = GccCompilerIdentityV1::new("15.2.0", sha256(b"test compiler")).unwrap();
        let profile = scoop_target_data::linux_c_bridge_profile(target, compiler).unwrap();
        let probe = paths.command();
        let invocation = ValidatedCBridgeToolchainInvocation::new_linux(
            target,
            profile,
            paths.driver,
            paths.sysroot,
            paths.real_gcc,
            paths.search_path,
        )
        .unwrap();
        for command in [probe, invocation.driver_command()] {
            let actual = command
                .get_envs()
                .find_map(|(key, value)| (key == "REALGCC").then_some(value))
                .flatten();
            assert_eq!(actual, real_gcc.as_deref().map(Path::as_os_str));
        }
    }
}
