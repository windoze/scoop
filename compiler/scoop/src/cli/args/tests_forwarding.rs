use super::*;

fn parse(arguments: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(with_single_file_arguments(
        arguments.iter().map(OsString::from).collect(),
    ))
}

#[test]
fn single_file_starts_forwarding_at_first_program_positional() {
    for (tail, expected) in [
        (vec!["Alice", "Bob"], vec!["Alice", "Bob"]),
        (vec!["--", "Alice", "Bob"], vec!["Alice", "Bob"]),
        (vec!["--", "--help"], vec!["--help"]),
        (vec!["Alice", "--release"], vec!["Alice", "--release"]),
        (vec!["Alice", "--", "Bob"], vec!["Alice", "--", "Bob"]),
        (vec!["", "--help", "a b"], vec!["", "--help", "a b"]),
        (vec!["-", "--help"], vec!["-", "--help"]),
    ] {
        let mut arguments = vec!["scoop", "run", "file.scoop"];
        arguments.extend(tail);
        let Command::Run(run) = parse(&arguments).unwrap().command else {
            panic!("expected run")
        };
        assert_eq!(run.program_args, expected);
        assert!(!run.options.release);
    }
}

#[test]
fn known_runner_options_keep_their_meaning_before_forwarding() {
    for arguments in [
        vec!["scoop", "run", "--release", "file.scoop", "Alice"],
        vec!["scoop", "run", "file.scoop", "--release", "Alice"],
        vec!["scoop", "run", "file.scoop", "--release", "--", "Alice"],
    ] {
        let Command::Run(run) = parse(&arguments).unwrap().command else {
            panic!("expected run")
        };
        assert!(run.options.release);
        assert_eq!(run.program_args, ["Alice"]);
    }
    assert_eq!(
        parse(&["scoop", "run", "file.scoop", "--help"])
            .err()
            .unwrap()
            .kind(),
        clap::error::ErrorKind::DisplayHelp
    );
    assert!(parse(&["scoop", "run", "file.scoop", "--unknown"]).is_err());
}

#[test]
fn option_values_are_not_roots_or_program_arguments() {
    for options in [
        vec!["--target-dir", "other.scoop", "-j", "2"],
        vec!["--target-dir=other.scoop", "-j2"],
    ] {
        let mut arguments = vec!["scoop", "run"];
        arguments.extend(options);
        arguments.extend(["file.scoop", "Alice"]);
        let Command::Run(run) = parse(&arguments).unwrap().command else {
            panic!("expected run")
        };
        assert_eq!(run.options.root_input, Some(PathBuf::from("file.scoop")));
        assert_eq!(run.options.target_dir, Some(PathBuf::from("other.scoop")));
        assert_eq!(run.options.jobs.unwrap().get(), 2);
        assert_eq!(run.program_args, ["Alice"]);
    }
    assert!(
        parse(&[
            "scoop",
            "run",
            "--target-dir",
            "other.scoop",
            "cone",
            "Alice"
        ])
        .is_err()
    );
}

#[test]
fn manifest_and_default_root_keep_explicit_separators() {
    for root in ["cone", "Cone.toml"] {
        assert!(parse(&["scoop", "run", root, "Alice"]).is_err());
        assert!(parse(&["scoop", "run", root, "--", "Alice"]).is_ok());
    }
    let Command::Run(run) = parse(&["scoop", "run", "--", "Alice"]).unwrap().command else {
        panic!("expected run")
    };
    assert!(run.options.root_input.is_none());
    assert_eq!(run.program_args, ["Alice"]);
    assert!(parse(&["scoop", "build", "file.scoop", "Alice"]).is_err());
}

#[cfg(unix)]
#[test]
fn forwarding_preserves_non_utf8_arguments_and_attached_option_values() {
    use std::os::unix::ffi::OsStringExt;
    let value = OsString::from_vec(vec![0xff]);
    let options = OsString::from_vec(b"--target-dir=\xff.scoop".to_vec());
    let raw = vec![
        "scoop".into(),
        "run".into(),
        options,
        "file.scoop".into(),
        value.clone(),
        "--".into(),
    ];
    let Command::Run(run) = Cli::try_parse_from(with_single_file_arguments(raw))
        .unwrap()
        .command
    else {
        panic!("expected run")
    };
    assert_eq!(run.program_args, [value, "--".into()]);
    assert_eq!(
        run.options.target_dir,
        Some(PathBuf::from(OsString::from_vec(b"\xff.scoop".to_vec())))
    );
}

#[test]
fn global_stress_option_keeps_runner_forwarding_boundaries() {
    for arguments in [
        vec!["scoop", "--enable-gc-stress", "run", "file.scoop", "Alice"],
        vec!["scoop", "run", "--enable-gc-stress", "file.scoop", "Alice"],
        vec!["scoop", "run", "file.scoop", "--enable-gc-stress", "Alice"],
    ] {
        let cli = parse(&arguments).unwrap();
        assert!(cli.enable_gc_stress);
        let Command::Run(run) = cli.command else {
            panic!("expected run")
        };
        assert_eq!(run.program_args, ["Alice"]);
    }
    let cli = parse(&["scoop", "run", "file.scoop", "Alice", "--enable-gc-stress"]).unwrap();
    assert!(!cli.enable_gc_stress);
    let Command::Run(run) = cli.command else {
        panic!("expected run")
    };
    assert_eq!(run.program_args, ["Alice", "--enable-gc-stress"]);
}
