use super::*;

#[test]
fn linux_final_profiles_link_and_run_with_the_selected_libc_and_llvm_unwinder() {
    for (triple, mode, expected_mode) in [
        ("x86_64-unknown-linux-gnu", None, LinkMode::Dynamic),
        ("x86_64-unknown-linux-musl", None, LinkMode::Static),
        (
            "x86_64-unknown-linux-musl",
            Some(LinkMode::Dynamic),
            LinkMode::Dynamic,
        ),
    ] {
        let target = ResolvedTargetProfile::resolve(triple).unwrap();
        let profile = target
            .final_link_with(&FinalLinkOptions {
                mode,
                ..Default::default()
            })
            .unwrap();
        let ValidatedFinalLinkProfile::Linux(linux) = &profile else {
            panic!("expected Linux profile")
        };
        assert_eq!(linux.mode(), expected_mode);
        assert!(
            linux
                .input_paths()
                .any(|path| path.file_name().unwrap() == "libunwind.a")
        );
        assert!(
            !linux
                .input_paths()
                .any(|path| path.to_string_lossy().contains("libgcc_s")
                    || path.to_string_lossy().contains("libgcc_eh"))
        );
        assert!(profile.system_provider().is_err());
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.c");
        let object = directory.path().join("main.o");
        std::fs::write(&source, probe::SOURCE).unwrap();
        probe::run(
            linux
                .startup
                .object_compilation_command(&source, &object)
                .args(["-funwind-tables", "-fno-omit-frame-pointer", "-I"])
                .arg(linux.unwind_prefix.join("include")),
        )
        .unwrap();
        let binary = directory.path().join("program");
        let mut command = linux
            .command(
                directory.path(),
                &binary,
                &directory.path().join("link.map"),
            )
            .unwrap();
        command.arg(object);
        linux.append_system_libraries(&mut command);
        probe::run(&mut command).unwrap();
        let bytes = std::fs::read(&binary).unwrap();
        linux.check_image(&bytes).unwrap();
        let output = probe::run(&mut Command::new(binary)).unwrap();
        assert_eq!(output.stdout, b"LLVM unwinder and target libc passed\n");
        assert_eq!(
            profile.fingerprint().unwrap(),
            target
                .final_link_with(&FinalLinkOptions {
                    mode,
                    ..Default::default()
                })
                .unwrap()
                .fingerprint()
                .unwrap()
        );
    }
}

#[test]
fn linux_final_profiles_reject_missing_unwind_and_unsupported_link_modes() {
    let target = ResolvedTargetProfile::resolve("x86_64-linux-gnu").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let error = target
        .final_link_with(&FinalLinkOptions {
            unwind_prefix: Some(directory.path().to_owned()),
            ..Default::default()
        })
        .unwrap_err();
    assert!(
        error.to_string().contains("missing LLVM unwind headers"),
        "{error}"
    );
    let error = target
        .final_link_with(&FinalLinkOptions {
            mode: Some(LinkMode::Static),
            ..Default::default()
        })
        .unwrap_err();
    assert!(error.to_string().contains("glibc static"), "{error}");
}
