use super::*;
use scoop_hir::CompileTimePath;
use scoop_identity::{ConeIdentity, NormalizedSourcePath, SourceIdentity};
use std::{fs, path::Path, sync::Arc};

fn identity() -> SourceIdentity {
    SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/main.scoop").unwrap(),
    )
}

fn input_host(root: &Path) -> CompileTimeInputs {
    CompileTimeInputs::default()
        .with_sources(root.to_owned(), [(identity(), root.join("src/main.scoop"))])
}

fn read(host: &CompileTimeInputs, path: &str) -> Result<Arc<[u8]>, String> {
    host.file(&identity(), &CompileTimePath::parse(path).unwrap())
}

fn setup() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("cone/src")).unwrap();
    fs::write(temp.path().join("cone/src/main.scoop"), "fun main() {}\n").unwrap();
    temp
}

#[test]
fn include_file_definition_location_bytes_and_updates() {
    let temp = setup();
    let root = fs::canonicalize(temp.path().join("cone")).unwrap();
    let resource = root.join("data.txt");
    fs::write(&resource, b"\xef\xbb\xbfhello\r\n\0").unwrap();
    let host = input_host(&root);
    let bytes = read(&host, "../data.txt").unwrap();
    assert_eq!(&*bytes, b"\xef\xbb\xbfhello\r\n\0");
    assert!(Arc::ptr_eq(&bytes, &read(&host, "../data.txt").unwrap()));
    let inputs = host.compiler_inputs();
    assert_eq!(inputs.len(), 1);
    assert!(host.inputs_current());
    fs::write(root.join("unused.txt"), "unobserved").unwrap();
    assert!(compiler_inputs_current(&inputs));
    fs::write(&resource, "changed").unwrap();
    assert!(!host.inputs_current());
    assert_eq!(&*read(&host, "../data.txt").unwrap(), &*bytes);
    fs::remove_file(resource).unwrap();
    assert!(!compiler_inputs_current(&inputs));
}

#[test]
fn include_file_rejects_outside_directories_and_missing_inputs() {
    let temp = setup();
    let root = fs::canonicalize(temp.path().join("cone")).unwrap();
    fs::write(root.parent().unwrap().join("outside"), "outside").unwrap();
    let host = input_host(&root);
    for (path, diagnostic) in [
        ("../../outside", "outside the defining Cone"),
        (".", "must name a regular file"),
        ("missing", "cannot read include input"),
    ] {
        assert!(read(&host, path).unwrap_err().contains(diagnostic));
    }
    assert!(host.compiler_inputs().is_empty());
    assert!(!host.inputs_current());
    fs::write(root.join("src/missing"), "now present").unwrap();
    assert_eq!(
        &*read(&input_host(&root), "missing").unwrap(),
        b"now present"
    );
}

#[cfg(unix)]
#[test]
fn include_file_checks_symlink_targets_and_original_dotdot_resolution() {
    use std::os::unix::fs::symlink;
    let temp = setup();
    let root = fs::canonicalize(temp.path().join("cone")).unwrap();
    fs::create_dir_all(root.join("inside/deep")).unwrap();
    fs::write(root.join("inside/data"), "correct").unwrap();
    fs::write(root.join("src/data"), "wrong").unwrap();
    symlink("../inside/deep", root.join("src/link")).unwrap();
    symlink("link", root.join("src/chain")).unwrap();
    let host = input_host(&root);
    assert_eq!(&*read(&host, "chain/../data").unwrap(), b"correct");
    let outside = root.parent().unwrap().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("data"), "outside").unwrap();
    symlink(&outside, root.join("src/escape")).unwrap();
    assert!(
        read(&host, "escape/data")
            .unwrap_err()
            .contains("symlink target")
    );
    symlink(root.join("inside/data"), outside.join("inward")).unwrap();
    assert!(
        read(&host, outside.join("inward").to_str().unwrap())
            .unwrap_err()
            .contains("include path")
    );
    symlink("missing", root.join("src/broken")).unwrap();
    assert!(read(&host, "broken").is_err());
}

#[cfg(unix)]
#[test]
fn include_file_root_alias_and_retarget_with_identical_metadata() {
    use std::os::unix::fs::symlink;
    let temp = setup();
    let root = fs::canonicalize(temp.path().join("cone")).unwrap();
    let alias = root.parent().unwrap().join("alias");
    symlink(&root, &alias).unwrap();
    for (name, bytes) in [("one", "one"), ("two", "two")] {
        fs::write(root.join(name), bytes).unwrap();
        fs::File::options()
            .write(true)
            .open(root.join(name))
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(100))
            .unwrap();
    }
    symlink("../one", root.join("src/resource")).unwrap();
    let host = input_host(&alias);
    assert_eq!(
        &*read(&host, alias.join("one").to_str().unwrap()).unwrap(),
        b"one"
    );
    assert_eq!(&*read(&host, "resource").unwrap(), b"one");
    assert!(host.inputs_current());
    fs::remove_file(root.join("src/resource")).unwrap();
    symlink("../two", root.join("src/resource")).unwrap();
    assert!(!host.inputs_current());
    assert_eq!(&*read(&input_host(&alias), "resource").unwrap(), b"two");
}
