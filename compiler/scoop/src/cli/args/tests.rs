use super::*;

#[test]
fn slib_output_and_test_artifacts_are_explicit_build_choices() {
    let cli = Cli::try_parse_from(["scoop", "build", "--emit-slib", "--tests"]).unwrap();
    let Command::Build(args) = cli.command else {
        panic!("expected build command")
    };
    assert!(args.emit_slib && args.tests);
    assert!(Cli::try_parse_from(["scoop", "build", "--tests"]).is_err());
    for command in ["run", "test", "check"] {
        assert!(Cli::try_parse_from(["scoop", command, "--emit-slib"]).is_err());
    }
}

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
fn independent_debug_files_are_an_explicit_final_link_option() {
    for command in ["build", "run", "test"] {
        assert!(Cli::try_parse_from(["scoop", command, "--release", "--emit-debug-file"]).is_ok());
    }
    assert!(
        Cli::try_parse_from([
            "scoop",
            "link",
            "--root-slib",
            "root.slib",
            "-o",
            "program",
            "--emit-debug-file"
        ])
        .is_ok()
    );
    assert!(Cli::try_parse_from(["scoop", "check", "--emit-debug-file"]).is_err());
}

#[test]
fn debug_information_is_an_independent_source_compilation_option() {
    for command in ["build", "run", "test"] {
        for level in ["none", "line-tables", "full"] {
            let cli = Cli::try_parse_from(["scoop", command, "--release", "--debug-info", level])
                .unwrap();
            let options = match cli.command {
                Command::Build(args) => args.options,
                Command::Run(args) => args.options,
                Command::Test(args) => args.options,
                _ => panic!("source compilation command"),
            };
            assert!(options.release);
            assert_eq!(options.debug_info.unwrap().as_str(), level);
        }
    }
    assert!(Cli::try_parse_from(["scoop", "build", "--debug-info", "invalid"]).is_err());
    assert!(Cli::try_parse_from(["scoop", "link", "--debug-info", "line-tables"]).is_err());
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
