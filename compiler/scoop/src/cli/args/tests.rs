use super::*;

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
