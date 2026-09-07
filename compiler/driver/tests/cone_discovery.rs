//! Source discovery tests (DESIGN section 1.3).

use std::fs;
use std::path::{Path, PathBuf};

use scoopc::cone::{ConeInputError, ConeSourceFile, ResolvedConeInput, discover_cone};

fn temp_cone(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("scoop-cone-test-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    dir
}

fn write_manifest(dir: &Path, kind: &str) {
    fs::write(
        dir.join("Cone.toml"),
        format!(
            "schema = 1\n\n[cone]\ngroup = \"dev.example\"\nname = \"app\"\nversion = \"0.1.0\"\nkind = \"{kind}\"\n"
        ),
    )
    .unwrap();
}

#[test]
fn discovers_nested_sources_in_byte_order() {
    let dir = temp_cone("nested");
    write_manifest(&dir, "library");
    // Create out of sorted order to prove discovery sorts.
    fs::write(dir.join("src/zebra.scoop"), "val z = 1\n").unwrap();
    fs::create_dir_all(dir.join("src/b/sub")).unwrap();
    fs::write(dir.join("src/b/sub/deep.scoop"), "// deep\n").unwrap();
    fs::write(dir.join("src/apple.scoop"), "// apple\n").unwrap();
    fs::write(dir.join("src/b/notes.txt"), "ignored").unwrap();
    fs::write(dir.join("src/readme.md"), "ignored").unwrap();

    let input = discover_cone(&dir).unwrap();
    let paths: Vec<&str> = input.sources().iter().map(|s| s.relative_path()).collect();
    assert_eq!(paths, ["apple.scoop", "b/sub/deep.scoop", "zebra.scoop"]);
    assert_eq!(
        input.manifest().coordinate().display(),
        "dev.example:app:0.1.0"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn empty_source_set_is_rejected() {
    let dir = temp_cone("empty");
    write_manifest(&dir, "library");
    assert_eq!(
        discover_cone(&dir).unwrap_err(),
        ConeInputError::EmptySourceSet
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn missing_manifest_and_src_are_rejected() {
    let dir =
        std::env::temp_dir().join(format!("scoop-cone-test-nomanifest-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    assert_eq!(
        discover_cone(&dir).unwrap_err(),
        ConeInputError::MissingManifest
    );

    let dir2 = temp_cone("nosrc");
    fs::write(
        dir2.join("Cone.toml"),
        "schema = 1\n[cone]\ngroup=\"a\"\nname=\"b\"\nversion=\"0.1.0\"\nkind=\"library\"\n",
    )
    .unwrap();
    fs::remove_dir_all(dir2.join("src")).unwrap();
    assert_eq!(
        discover_cone(&dir2).unwrap_err(),
        ConeInputError::MissingSourceRoot
    );
    fs::remove_dir_all(&dir).unwrap();
    fs::remove_dir_all(&dir2).unwrap();
}

#[test]
fn symlinks_are_rejected() {
    let dir = temp_cone("symlink");
    write_manifest(&dir, "library");
    fs::write(dir.join("outside.scoop"), "val x = 1\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.join("outside.scoop"), dir.join("src/link.scoop")).unwrap();
    #[cfg(not(unix))]
    fs::write(dir.join("src/link.scoop"), "// placeholder\n").unwrap();
    let error = discover_cone(&dir).unwrap_err();
    #[cfg(unix)]
    assert!(matches!(error, ConeInputError::SymlinkInSources(_)));
    #[cfg(not(unix))]
    assert_eq!(error, ConeInputError::EmptySourceSet);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn synthetic_input_round_trip_and_duplicates() {
    let manifest = scoop_manifest::parse_manifest(
        "schema = 1\n[cone]\ngroup=\"a\"\nname=\"b\"\nversion=\"0.1.0\"\nkind=\"executable\"\n",
    )
    .unwrap();
    let a = ConeSourceFile::new("main.scoop".to_owned(), "fun main() {}".to_owned()).unwrap();
    let b = ConeSourceFile::new("util/x.scoop".to_owned(), "// x".to_owned()).unwrap();
    // Supplied unsorted; from_parts normalizes.
    let input = ResolvedConeInput::from_parts(manifest.clone(), vec![b, a]).unwrap();
    let paths: Vec<&str> = input.sources().iter().map(|s| s.relative_path()).collect();
    assert_eq!(paths, ["main.scoop", "util/x.scoop"]);

    let dup1 = ConeSourceFile::new("a.scoop".to_owned(), "// 1".to_owned()).unwrap();
    let dup2 = ConeSourceFile::new("a.scoop".to_owned(), "// 2".to_owned()).unwrap();
    assert_eq!(
        ResolvedConeInput::from_parts(manifest, vec![dup1, dup2]).unwrap_err(),
        ConeInputError::DuplicatePath("a.scoop".to_owned())
    );
}

#[test]
fn relative_path_validation() {
    assert!(ConeSourceFile::new("a.scoop".to_owned(), String::new()).is_ok());
    assert!(ConeSourceFile::new("a/b.scoop".to_owned(), String::new()).is_ok());
    for bad in [
        "",
        "/a.scoop",
        "a/",
        "a//b.scoop",
        "./a.scoop",
        "../a.scoop",
        "a/./b.scoop",
        "a/../b.scoop",
        "a.txt",
        ".scoop",
        "a/.scoop",
    ] {
        assert!(
            ConeSourceFile::new(bad.to_owned(), String::new()).is_err(),
            "path {bad:?} should be rejected"
        );
    }
}

#[test]
fn source_text_is_preserved_verbatim() {
    let dir = temp_cone("text");
    write_manifest(&dir, "library");
    let text = "fun f(): Int {\n    return 1\n}\n";
    fs::write(dir.join("src/f.scoop"), text).unwrap();
    let input = discover_cone(&dir).unwrap();
    assert_eq!(input.sources()[0].text(), text);
    fs::remove_dir_all(&dir).unwrap();
}
