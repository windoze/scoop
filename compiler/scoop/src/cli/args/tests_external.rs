use super::*;

#[test]
fn external_commands_forward_every_argument_after_the_name() {
    for tail in [
        vec![],
        vec!["--help", "--version", "--", "", "two words"],
        vec!["--sysroot", "child-root", "--enable-gc-stress", "--unknown"],
    ] {
        let mut raw = vec!["scoop", "--sysroot", "parent-root", "probe"];
        raw.extend(&tail);
        let cli = Cli::try_parse_from(raw).unwrap();
        assert_eq!(cli.sysroot, Some(PathBuf::from("parent-root")));
        assert!(!cli.enable_gc_stress);
        let Command::External(args) = cli.command else {
            panic!("expected external command")
        };
        assert_eq!(args[0], "probe");
        assert_eq!(args[1..], tail);
    }
}

#[test]
fn sysroot_is_global_and_builtin_validation_keeps_precedence() {
    for command in ["build", "run", "test", "doc"] {
        for raw in [
            vec!["scoop", "--sysroot", "selected-root", command],
            vec!["scoop", command, "--sysroot", "selected-root"],
        ] {
            let cli = Cli::try_parse_from(raw).unwrap();
            assert_eq!(cli.sysroot, Some(PathBuf::from("selected-root")));
            assert!(!matches!(cli.command, Command::External(_)));
        }
        assert!(Cli::try_parse_from(["scoop", command, "--unknown"]).is_err());
    }
    let cli = Cli::try_parse_from(with_single_file_arguments(
        ["scoop", "--sysroot", "run", "run", "file.scoop", "Alice"]
            .map(OsString::from)
            .to_vec(),
    ))
    .unwrap();
    assert_eq!(cli.sysroot, Some(PathBuf::from("run")));
    let Command::Run(args) = cli.command else {
        panic!("expected run command")
    };
    assert_eq!(args.program_args, ["Alice"]);
}

#[cfg(unix)]
#[test]
fn external_arguments_and_sysroot_preserve_non_utf8_bytes() {
    use std::os::unix::ffi::OsStringExt;

    let root = OsString::from_vec(b"root-\xff".to_vec());
    let argument = OsString::from_vec(vec![0xff]);
    let cli = Cli::try_parse_from(with_single_file_arguments(vec![
        "scoop".into(),
        "--sysroot".into(),
        root.clone(),
        "probe".into(),
        argument.clone(),
    ]))
    .unwrap();
    assert_eq!(cli.sysroot, Some(PathBuf::from(root)));
    let Command::External(args) = cli.command else {
        panic!("expected external command")
    };
    assert_eq!(args, [OsString::from("probe"), argument]);
}
