use super::*;
use scoop_hir::{CompileTimeDirectoryError as Error, CompileTimeDirectoryFilter, CompileTimePath};
use scoop_identity::{ConeIdentity, NormalizedSourcePath, SourceIdentity};
use std::{fs, path::Path, sync::Arc};

fn identity() -> SourceIdentity {
    SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/main.scoop").unwrap(),
    )
}

fn host(root: &Path) -> CompileTimeInputs {
    CompileTimeInputs::default()
        .with_sources(root.to_owned(), [(identity(), root.join("src/main.scoop"))])
}

fn filter(include: &[&str], exclude: &[&str]) -> CompileTimeDirectoryFilter {
    CompileTimeDirectoryFilter {
        include: include.iter().map(|value| (*value).into()).collect(),
        exclude: exclude.iter().map(|value| (*value).into()).collect(),
    }
}

fn read(
    host: &CompileTimeInputs,
    path: &str,
    filter: &CompileTimeDirectoryFilter,
) -> Result<Arc<[scoop_hir::CompileTimeFile]>, Error> {
    host.directory(&identity(), &CompileTimePath::parse(path).unwrap(), filter)
}

fn setup() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("cone/src")).unwrap();
    fs::create_dir_all(temp.path().join("cone/assets/sub")).unwrap();
    fs::write(temp.path().join("cone/src/main.scoop"), "fun main() {}").unwrap();
    temp
}

#[test]
fn directory_inputs_sort_select_raw_bytes_and_reuse_one_read() {
    let temp = setup();
    let root = fs::canonicalize(temp.path().join("cone")).unwrap();
    for (name, bytes) in [
        (".hidden.bin", b"\xff".as_slice()),
        ("a.txt", b"text"),
        ("sub/z.bin", b"\0\x80"),
        ("sub/ignored.txt", b"ignored"),
    ] {
        fs::write(root.join("assets").join(name), bytes).unwrap();
    }
    let host = host(&root);
    let filter = filter(&["**/*.bin", "*.bin", "?.txt"], &[]);
    let files = read(&host, "../assets", &filter).unwrap();
    assert_eq!(
        files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        [".hidden.bin", "a.txt", "sub/z.bin"]
    );
    assert_eq!(&*files[0].bytes, b"\xff");
    assert_eq!(&*files[2].bytes, b"\0\x80");
    assert!(Arc::ptr_eq(
        &files,
        &read(&host, "../assets", &filter).unwrap()
    ));
    let recorded = host.compiler_inputs();
    assert_eq!(recorded.len(), 1);
    assert!(host.inputs_current());
    fs::write(root.join("assets/sub/ignored.txt"), "changed but excluded").unwrap();
    assert!(compiler_inputs_current(&recorded));
    fs::write(root.join("assets/sub/z.bin"), "changed").unwrap();
    assert!(!compiler_inputs_current(&recorded));
    assert_eq!(
        &*read(&host, "../assets", &filter).unwrap()[2].bytes,
        b"\0\x80"
    );
}

#[test]
fn directory_updates_discover_deep_additions_renames_removals_and_empty_directories() {
    let temp = setup();
    let root = fs::canonicalize(temp.path().join("cone")).unwrap();
    let filter = filter(&["**/*.bin"], &[]);
    let observe = || {
        let host = host(&root);
        read(&host, "../assets", &filter).unwrap();
        host.compiler_inputs()
    };
    let before = observe();
    fs::write(root.join("assets/sub/new.txt"), "not selected").unwrap();
    assert!(!compiler_inputs_current(&before));
    let before = observe();
    fs::rename(
        root.join("assets/sub/new.txt"),
        root.join("assets/sub/new.bin"),
    )
    .unwrap();
    assert!(!compiler_inputs_current(&before));
    let before = observe();
    fs::remove_file(root.join("assets/sub/new.bin")).unwrap();
    assert!(!compiler_inputs_current(&before));
    let before = observe();
    fs::create_dir(root.join("assets/sub/empty")).unwrap();
    assert!(!compiler_inputs_current(&before));
}

#[test]
fn directory_errors_locate_globs_and_missing_inputs_can_recover() {
    let temp = setup();
    let root = fs::canonicalize(temp.path().join("cone")).unwrap();
    let input = host(&root);
    assert!(matches!(
        read(&input, "../missing", &filter(&["**", "bad["], &[])),
        Err(Error::IncludePattern { index: 1, .. })
    ));
    assert!(matches!(
        read(&input, "../assets", &filter(&["**"], &["x", "**bad"])),
        Err(Error::ExcludePattern { index: 1, .. })
    ));
    assert!(read(&input, "main.scoop", &filter(&[], &[])).is_err());
    assert!(read(&input, "../missing", &filter(&[], &[])).is_err());
    assert!(!input.inputs_current());
    fs::create_dir(root.join("missing")).unwrap();
    assert!(
        read(&host(&root), "../missing", &filter(&[], &[]))
            .unwrap()
            .is_empty()
    );
    assert!(
        read(&host(&root), "../assets", &filter(&[], &[]))
            .unwrap()
            .is_empty()
    );
}

#[cfg(unix)]
#[test]
fn directory_checks_excluded_symlinks_cycles_and_non_regular_entries() {
    use std::os::unix::{fs::symlink, net::UnixListener};
    let temp = setup();
    let root = fs::canonicalize(temp.path().join("cone")).unwrap();
    let outside = root.parent().unwrap().join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, root.join("assets/escape")).unwrap();
    for selection in [filter(&[], &[]), filter(&["**"], &["escape/**"])] {
        let error = read(&host(&root), "../assets", &selection).unwrap_err();
        assert!(
            matches!(error, Error::Input(message) if message.contains("entry `escape`") && message.contains("outside the defining Cone"))
        );
    }
    fs::remove_file(root.join("assets/escape")).unwrap();
    symlink(".", root.join("assets/loop")).unwrap();
    assert!(
        matches!(read(&host(&root), "../assets", &filter(&[], &[])), Err(Error::Input(message)) if message.contains("directory cycle"))
    );
    fs::remove_file(root.join("assets/loop")).unwrap();
    symlink("missing", root.join("assets/broken")).unwrap();
    assert!(read(&host(&root), "../assets", &filter(&[], &[])).is_err());
    fs::remove_file(root.join("assets/broken")).unwrap();
    let _socket = UnixListener::bind(root.join("assets/socket")).unwrap();
    assert!(
        read(&host(&root), "../assets", &filter(&[], &[]))
            .unwrap()
            .is_empty()
    );
    assert!(
        matches!(read(&host(&root), "../assets", &filter(&["**"], &[])), Err(Error::Input(message)) if message.contains("must name a regular file"))
    );
}

#[cfg(unix)]
#[test]
fn directory_links_preserve_logical_paths_and_detect_equal_metadata_retargets() {
    use std::os::unix::fs::symlink;
    let temp = setup();
    let root = fs::canonicalize(temp.path().join("cone")).unwrap();
    for (directory, contents) in [("one", "one"), ("two", "two")] {
        fs::create_dir(root.join(directory)).unwrap();
        let path = root.join(directory).join("data.bin");
        fs::write(&path, contents).unwrap();
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(100))
            .unwrap();
    }
    symlink("../one", root.join("assets/a")).unwrap();
    symlink("../one", root.join("assets/b")).unwrap();
    let input = host(&root);
    let files = read(&input, "../assets", &filter(&["**"], &[])).unwrap();
    assert_eq!(
        files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        ["a/data.bin", "b/data.bin"]
    );
    assert!(Arc::ptr_eq(&files[0].bytes, &files[1].bytes));
    let before = input.compiler_inputs();
    fs::remove_file(root.join("assets/a")).unwrap();
    symlink("../two", root.join("assets/a")).unwrap();
    assert!(!compiler_inputs_current(&before));
    let files = read(&host(&root), "../assets", &filter(&["**"], &[])).unwrap();
    assert_eq!(&*files[0].bytes, b"two");
    assert_eq!(&*files[1].bytes, b"one");
}

#[cfg(unix)]
#[test]
fn directory_root_bounds_keep_original_link_dotdot_resolution() {
    use std::os::unix::fs::symlink;
    let temp = setup();
    let root = fs::canonicalize(temp.path().join("cone")).unwrap();
    fs::create_dir_all(root.join("other/deep")).unwrap();
    fs::create_dir(root.join("other/assets")).unwrap();
    fs::write(root.join("other/assets/right.bin"), "right").unwrap();
    symlink("../other/deep", root.join("src/link")).unwrap();
    let files = read(&host(&root), "link/../assets", &filter(&["**"], &[])).unwrap();
    assert_eq!(files[0].path, "right.bin");
    let alias = root.parent().unwrap().join("alias");
    symlink(&root, &alias).unwrap();
    assert!(
        read(
            &host(&alias),
            alias.join("assets").to_str().unwrap(),
            &filter(&["**"], &[])
        )
        .is_ok()
    );
    let outside = root.parent().unwrap().join("inward");
    symlink(root.join("assets"), &outside).unwrap();
    assert!(
        matches!(read(&host(&root), outside.to_str().unwrap(), &filter(&["**"], &[])), Err(Error::Input(message)) if message.contains("include path") && message.contains("outside the defining Cone"))
    );
}

// Darwin filesystems reject invalid Unicode filenames before the compiler can observe them.
#[cfg(target_os = "linux")]
#[test]
fn directory_rejects_non_utf8_names_even_when_not_selected() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let temp = setup();
    let root = fs::canonicalize(temp.path().join("cone")).unwrap();
    fs::write(
        root.join("assets").join(OsString::from_vec(vec![255])),
        "bytes",
    )
    .unwrap();
    assert!(
        matches!(read(&host(&root), "../assets", &filter(&[], &[])), Err(Error::Input(message)) if message.contains("valid UTF-8 path"))
    );
}
