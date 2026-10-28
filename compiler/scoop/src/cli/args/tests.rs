use super::*;

#[test]
fn clean_is_builtin_and_accepts_only_cleanup_options() {
    assert!(matches!(
        Cli::try_parse_from([
            "scoop",
            "clean",
            "project/Cone.toml",
            "--target-dir",
            "out",
            "--message-format",
            "json"
        ])
        .unwrap()
        .command,
        Command::Clean(_)
    ));
    for option in [
        "--release",
        "--target",
        "--cache-dir",
        "--scoopc",
        "--emit-symbol-map",
    ] {
        assert!(Cli::try_parse_from(["scoop", "clean", option]).is_err());
    }
}

#[test]
fn symbol_maps_are_an_explicit_final_link_option() {
    for command in ["build", "run", "test"] {
        assert!(Cli::try_parse_from(["scoop", command, "--release", "--emit-symbol-map"]).is_ok());
    }
    assert!(
        Cli::try_parse_from([
            "scoop",
            "link",
            "--root-slib",
            "root.slib",
            "-o",
            "program",
            "--emit-symbol-map"
        ])
        .is_ok()
    );
}

#[test]
fn output_capture_only_applies_to_actual_execution() {
    for flag in ["--list", "--no-run"] {
        assert!(Cli::try_parse_from(["scoop", "test", flag, "--nocapture"]).is_err());
    }
    assert!(Cli::try_parse_from(["scoop", "test", "--nocapture", "--ignored"]).is_ok());
}

#[test]
fn documentation_accepts_multiple_roots_and_render_options() {
    let cli = Cli::try_parse_from([
        "scoop",
        "doc",
        "library",
        "application/Cone.toml",
        "--no-deps",
        "--document-private-items",
        "--deny-doc-warnings",
        "--open",
        "--output-dir",
        "site",
        "--message-format",
        "json",
    ])
    .unwrap();
    let Command::Doc(args) = cli.command else {
        panic!("expected doc command")
    };
    assert_eq!(
        args.roots,
        [
            PathBuf::from("library"),
            PathBuf::from("application/Cone.toml")
        ]
    );
    assert_eq!(args.output_dir, Some(PathBuf::from("site")));
    assert!(args.no_deps && args.document_private_items && args.deny_doc_warnings && args.open);
}

#[test]
fn documentation_defaults_to_current_cone_and_has_no_final_link_options() {
    let cli = Cli::try_parse_from(["scoop", "doc"]).unwrap();
    let Command::Doc(args) = cli.command else {
        panic!("expected doc command")
    };
    assert!(args.roots.is_empty());
    for flag in [
        "--runtime",
        "--unwind-library-path",
        "--release",
        "--nocapture",
    ] {
        assert!(Cli::try_parse_from(["scoop", "doc", flag]).is_err());
    }
}

#[test]
fn compiler_jobs_apply_to_build_run_and_test_and_must_be_positive() {
    for command in ["build", "run", "test"] {
        assert!(Cli::try_parse_from(["scoop", command, "-j", "2"]).is_ok());
        assert!(Cli::try_parse_from(["scoop", command, "--jobs", "1"]).is_ok());
        assert!(Cli::try_parse_from(["scoop", command, "--jobs", "0"]).is_err());
    }
}

#[test]
fn package_selection_and_lock_flags_are_shared_by_build_commands() {
    for command in ["build", "run", "test", "check", "doc"] {
        assert!(
            Cli::try_parse_from([
                "scoop",
                command,
                "-p",
                "example:one",
                "-p",
                "example:two",
                "--frozen",
                "--lockfile",
                "locks/Cone.lock",
                "--package-cache",
                "shared-packages",
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from(["scoop", command, "--workspace", "-p", "example:one"]).is_err()
        );
    }
}
