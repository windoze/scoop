use super::*;
use crate::cli::args::{Cli, Command};
use clap::Parser;

#[test]
fn group_and_path_are_required_without_build_options() {
    for arguments in [
        vec!["scoop", "new", "hello"],
        vec!["scoop", "new", "--group", "dev.example"],
        vec![
            "scoop",
            "new",
            "hello",
            "--group",
            "dev.example",
            "--release",
        ],
    ] {
        assert!(Cli::try_parse_from(arguments).is_err());
    }
}

#[test]
fn version_is_a_template_value_and_named_package_segments_are_not_string_prefixes() {
    let cli = Cli::try_parse_from([
        "scoop",
        "new",
        "utilities",
        "--group",
        "dev.example",
        "--version",
        "1.2.3",
        "--package",
        "dev.examples",
        "--kind",
        "library",
    ])
    .unwrap();
    let Command::New(mut args) = cli.command else {
        panic!("expected new")
    };
    assert_eq!(args.version, "1.2.3");
    assert!(template(&args).is_err());
    args.package = Some("`dev.example`".into());
    assert!(template(&args).is_err());
    args.package = Some("dev.example.tools".into());
    assert!(template(&args).is_ok());
}

#[test]
fn invalid_options_do_not_create_parent_directories() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("missing/child");
    let cli = Cli::try_parse_from([
        "scoop",
        "new",
        path.to_str().unwrap(),
        "--group",
        "Uppercase",
    ])
    .unwrap();
    let Command::New(args) = cli.command else {
        panic!("expected new")
    };
    assert!(execute(args).is_err());
    assert!(!temp.path().join("missing").exists());
}
