use super::super::args::{Cli, Command};
use clap::Parser;

#[test]
fn fmt_is_builtin_and_accepts_multiple_inputs_without_build_options() {
    let Command::Fmt(args) = Cli::try_parse_from([
        "scoop",
        "fmt",
        "a",
        "b.scoop",
        "--check",
        "--message-format",
        "json",
    ])
    .unwrap()
    .command
    else {
        panic!("fmt is builtin");
    };
    assert_eq!(args.inputs.len(), 2);
    assert!(args.check);
    assert!(Cli::try_parse_from(["scoop", "fmt"]).is_ok());
    for flag in [
        "--target",
        "--profile",
        "--cache-dir",
        "--generator",
        "-o",
        "--scoopc",
        "--jobs",
    ] {
        assert!(Cli::try_parse_from(["scoop", "fmt", flag, "value"]).is_err());
    }
    assert!(
        Cli::try_parse_from(["scoop", "--sysroot", "ignored", "fmt", "--enable-gc-stress"]).is_ok()
    );
}

#[cfg(unix)]
#[test]
fn write_preserves_permissions_links_and_unchanged_mtime() {
    use super::super::{args::MessageFormat, presentation::Reporter};
    use super::{FileStatus, expand, format_file};
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.scoop");
    let alias = directory.path().join("alias.scoop");
    std::fs::write(&path, "fun f()=1").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o751)).unwrap();
    std::os::unix::fs::symlink("source.scoop", &alias).unwrap();
    let selected = expand(
        directory.path(),
        Some(alias.clone()),
        &Default::default(),
        None,
    )
    .unwrap();
    let reporter = Reporter(MessageFormat::Json);
    assert_eq!(
        format_file(&selected[0], true, &reporter).unwrap(),
        FileStatus::NeedsFormat
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "fun f()=1");
    assert_eq!(
        format_file(&selected[0], false, &reporter).unwrap(),
        FileStatus::Formatted
    );
    let metadata = std::fs::metadata(&path).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o777, 0o751);
    assert!(
        std::fs::symlink_metadata(&alias)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        format_file(&selected[0], false, &reporter).unwrap(),
        FileStatus::Unchanged
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        metadata.modified().unwrap()
    );
    assert_eq!(std::fs::read_to_string(&alias).unwrap(), "fun f() = 1\n");
}

#[cfg(target_os = "linux")]
#[test]
fn explicit_non_utf8_path_is_preserved_through_writeback() {
    use super::super::{args::MessageFormat, presentation::Reporter};
    use super::{FileStatus, expand, format_file};
    use std::os::unix::ffi::OsStringExt;
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .join(std::ffi::OsString::from_vec(b"source-\xff.scoop".to_vec()));
    std::fs::write(&path, "fun f()=1").unwrap();
    let selected = expand(
        directory.path(),
        Some(path.clone()),
        &Default::default(),
        None,
    )
    .unwrap();
    assert_eq!(selected, [std::fs::canonicalize(&path).unwrap()]);
    assert_eq!(
        format_file(&selected[0], false, &Reporter(MessageFormat::Json)).unwrap(),
        FileStatus::Formatted
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "fun f() = 1\n");
}
