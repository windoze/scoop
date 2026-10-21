use super::*;

#[test]
fn warm_headers_and_body_updates_preserve_defaults_and_changed_deps() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("script.scoop");
    std::fs::write(&path, "fun main() {}\n").unwrap();
    let locator = SingleFileLocator::from_path(&path).unwrap();
    let cache = ArtifactCacheRoot::new(temp.path().join("cache")).unwrap();
    let defaults = cache.load_single_file_header(&locator).unwrap();
    assert_eq!(defaults.len(), 2);
    assert_eq!(cache.load_single_file_header(&locator).unwrap(), defaults);
    let records = || {
        std::fs::read_dir(cache.as_path().join("single-file-headers-v1"))
            .unwrap()
            .filter(|entry| entry.as_ref().unwrap().path().extension().unwrap() == "json")
            .count()
    };
    assert_eq!(records(), 1);
    std::fs::write(&path, "fun main() { println(42) }\n").unwrap();
    assert_eq!(cache.load_single_file_header(&locator).unwrap(), defaults);
    assert_eq!(records(), 2);
    std::fs::write(&path, "//! dep: dev:library:1.0.0\nfun main() {}\n").unwrap();
    let explicit = cache.load_single_file_header(&locator).unwrap();
    assert_eq!(explicit.len(), 3);
    assert_eq!(&explicit[..2], defaults);
    std::fs::write(&path, "fun main() {}\n").unwrap();
    assert_eq!(cache.load_single_file_header(&locator).unwrap(), defaults);
}

#[test]
fn malformed_changed_header_is_located_and_never_cached_as_default() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("script.scoop");
    std::fs::write(&path, "//! dep:\n").unwrap();
    let locator = SingleFileLocator::from_path(&path).unwrap();
    let cache = ArtifactCacheRoot::new(temp.path().join("cache")).unwrap();
    for _ in 0..2 {
        let error = cache.load_single_file_header(&locator).unwrap_err();
        assert_eq!(error.path(), path);
        assert_eq!(error.span(), Some(0..8));
    }
    assert!(
        std::fs::read_dir(cache.as_path().join("single-file-headers-v1"))
            .unwrap()
            .all(|entry| entry.unwrap().path().extension().unwrap() == "lock")
    );
}
