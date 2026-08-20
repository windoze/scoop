use std::sync::atomic::{AtomicU64, Ordering};

use crate::{ManifestRootLocator, load_cone_manifest};

use super::walk::validated_entry_name;
use super::*;

mod selection;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempCone(PathBuf);

impl TempCone {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "scoop-manifest-discovery-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir_all(path.join("src")).unwrap();
        std::fs::write(
            path.join("Cone.toml"),
            "schema = 1\n[cone]\ngroup = \"dev.example\"\nname = \"lib\"\nversion = \"1.0.0\"\nkind = \"library\"\n",
        )
        .unwrap();
        Self(path)
    }

    fn load(&self) -> LoadedConeManifest {
        load_cone_manifest(&ManifestRootLocator::cone_directory(&self.0)).unwrap()
    }
}

impl Drop for TempCone {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn recursively_discovers_sources_in_logical_path_order() {
    let cone = TempCone::new();
    std::fs::create_dir(cone.0.join("src/z")).unwrap();
    std::fs::write(cone.0.join("src/z/b.scoop"), "package z\n").unwrap();
    std::fs::write(cone.0.join("src/a.scoop"), "package a\n").unwrap();
    std::fs::write(cone.0.join("src/ignored.txt"), "ignored").unwrap();

    let sources = discover_manifest_sources(&cone.load(), TargetProfileId::DarwinAarch64).unwrap();
    let paths = sources
        .iter()
        .map(|source| source.identity().logical_path().as_str())
        .collect::<Vec<_>>();
    assert_eq!(paths, ["src/a.scoop", "src/z/b.scoop"]);
    assert_eq!(
        sources.first().content_digest(),
        SourceContentDigest::from_utf8("package a\n")
    );
}

#[test]
fn rejects_an_empty_source_tree() {
    let cone = TempCone::new();
    assert!(matches!(
        discover_manifest_sources(&cone.load(), TargetProfileId::DarwinAarch64)
            .unwrap_err()
            .kind(),
        SourceDiscoveryErrorKind::EmptySourceSet
    ));
}

#[test]
fn rejects_invalid_utf8_source_text() {
    let cone = TempCone::new();
    std::fs::write(cone.0.join("src/bad.scoop"), [0xff]).unwrap();
    assert!(matches!(
        discover_manifest_sources(&cone.load(), TargetProfileId::DarwinAarch64)
            .unwrap_err()
            .kind(),
        SourceDiscoveryErrorKind::InvalidUtf8Source
    ));
}

#[cfg(unix)]
#[test]
fn rejects_distinct_logical_paths_to_the_same_real_source() {
    let cone = TempCone::new();
    let target = cone.0.join("src/shared.scoop");
    std::fs::write(&target, "fun shared() {}\n").unwrap();
    std::os::unix::fs::symlink("shared.scoop", cone.0.join("src/alias.scoop")).unwrap();

    assert!(matches!(
        discover_manifest_sources(&cone.load(), TargetProfileId::DarwinAarch64)
            .unwrap_err()
            .kind(),
        SourceDiscoveryErrorKind::ConflictingSelections { .. }
    ));
}

#[cfg(unix)]
#[test]
fn allows_a_source_symlink_within_the_cone_root() {
    let cone = TempCone::new();
    let outside = cone.0.join("outside.scoop");
    std::fs::write(&outside, "fun outside() {}\n").unwrap();
    std::os::unix::fs::symlink("../outside.scoop", cone.0.join("src/escape.scoop")).unwrap();

    let sources = discover_manifest_sources(&cone.load(), TargetProfileId::DarwinAarch64).unwrap();
    assert_eq!(
        sources.first().identity().logical_path().as_str(),
        "src/escape.scoop"
    );
}

#[cfg(unix)]
#[test]
fn rejects_a_directory_cycle() {
    let cone = TempCone::new();
    std::fs::write(cone.0.join("src/source.scoop"), "fun source() {}\n").unwrap();
    std::os::unix::fs::symlink(".", cone.0.join("src/again")).unwrap();

    assert!(matches!(
        discover_manifest_sources(&cone.load(), TargetProfileId::DarwinAarch64)
            .unwrap_err()
            .kind(),
        SourceDiscoveryErrorKind::DirectoryCycle
    ));
}

#[cfg(unix)]
#[test]
fn follows_long_acyclic_symlink_chains() {
    let cone = TempCone::new();
    let root = cone.0.join("src");
    std::fs::write(root.join("last.scoop"), "fun source() {}\n").unwrap();
    for index in 0..80 {
        let next = if index == 79 {
            "last.scoop".to_owned()
        } else {
            format!("link{}", index + 1)
        };
        std::os::unix::fs::symlink(next, root.join(format!("link{index}"))).unwrap();
    }
    let resolved = resolve_entry(&root.join("link0")).unwrap();
    assert_eq!(
        resolved.path,
        std::fs::canonicalize(root.join("last.scoop")).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn rejects_symlink_cycles_with_different_relative_spellings() {
    let cone = TempCone::new();
    let root = cone.0.join("src");
    std::os::unix::fs::symlink("./second", root.join("first")).unwrap();
    std::os::unix::fs::symlink("../src/first", root.join("second")).unwrap();
    assert!(matches!(
        resolve_entry(&root.join("first")).unwrap_err().kind(),
        SourceDiscoveryErrorKind::SymlinkCycle
    ));
}

#[cfg(unix)]
#[test]
fn rejects_non_utf8_directory_entries() {
    use std::os::unix::ffi::OsStringExt;

    let invalid = std::ffi::OsString::from_vec(vec![0xff]);
    assert!(validated_entry_name(&invalid).is_none());
}
