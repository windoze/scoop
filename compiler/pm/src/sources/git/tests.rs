use scoop_manifest::ConeRelativePath;

use super::*;

struct Repository {
    directory: tempfile::TempDir,
    cache: PackageCache,
    source: GitSource,
}

impl Repository {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("repository");
        git(Command::new("git").args(["init", "-b", "main"]).arg(&path)).unwrap();
        let cache =
            PackageCache::resolve(directory.path(), Some(&directory.path().join("cache"))).unwrap();
        let source = GitSource {
            repository: path.to_str().unwrap().into(),
            reference: GitReference::DefaultBranch,
            subdir: ConeRelativePath::new(".").unwrap(),
        };
        Self {
            directory,
            cache,
            source,
        }
    }

    fn commit(&self, value: &str) -> String {
        let path = Path::new(&self.source.repository);
        fs::write(path.join("value.txt"), value).unwrap();
        git(Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["add", "value.txt"]))
        .unwrap();
        git(Command::new("git").arg("-C").arg(path).args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-m",
            value,
        ]))
        .unwrap();
        git(Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "HEAD"]))
        .unwrap()
        .trim()
        .into()
    }

    fn checkout(&self, locked: Option<&str>, offline: bool) -> PmResult<GitCheckout> {
        checkout(
            &self.cache,
            &self.source,
            self.directory.path(),
            locked,
            offline,
        )
    }
}

#[test]
fn locked_checkouts_are_shared_and_only_unpinned_updates_follow_the_remote() {
    let repository = Repository::new();
    let first = repository.commit("first");
    let a = repository.checkout(None, false).unwrap();
    assert_eq!(a.commit, first);
    let second = repository.commit("second");
    let b = repository.checkout(Some(&first), true).unwrap();
    assert_eq!(a.root, b.root);
    assert_eq!(
        fs::read_to_string(b.root.join("value.txt")).unwrap(),
        "first"
    );
    assert_eq!(repository.checkout(None, true).unwrap().commit, first);
    let updated = repository.checkout(None, false).unwrap();
    assert_eq!(updated.commit, second);
    assert_ne!(updated.root, a.root);
    assert_eq!(
        fs::read_to_string(updated.root.join("value.txt")).unwrap(),
        "second"
    );
    assert_eq!(
        repository.checkout(Some(&first), false).unwrap().root,
        a.root
    );
}

#[test]
fn refs_and_subdirs_of_the_same_commit_share_one_checkout() {
    let mut repository = Repository::new();
    let first = repository.commit("first");
    git(Command::new("git")
        .arg("-C")
        .arg(&repository.source.repository)
        .args(["tag", "v1.0.0"]))
    .unwrap();
    repository.source.reference = GitReference::Tag("v1.0.0".into());
    let tag = repository.checkout(None, false).unwrap();
    repository.source.reference = GitReference::Branch("main".into());
    repository.source.subdir = ConeRelativePath::new("cones/a").unwrap();
    let branch = repository.checkout(Some(&first), false).unwrap();
    assert_eq!(tag.root, branch.root);
}

#[test]
fn offline_objects_can_materialize_an_uncached_commit_without_updating_refs() {
    let mut repository = Repository::new();
    let first = repository.commit("first");
    let second = repository.commit("second");
    repository.checkout(None, false).unwrap();
    repository.source.reference = GitReference::Revision(first.clone());
    let old = repository.checkout(Some(&first), true).unwrap();
    assert_eq!(old.commit, first);
    assert_eq!(
        fs::read_to_string(old.root.join("value.txt")).unwrap(),
        "first"
    );
    assert!(
        repository
            .checkout(Some(&"0".repeat(40)), true)
            .unwrap_err()
            .to_string()
            .contains("not cached")
    );
    assert!(
        repository
            .checkout(Some(&second[..12]), true)
            .unwrap_err()
            .to_string()
            .contains("full Git commit")
    );
}

#[test]
fn missing_refs_do_not_publish_checkouts_and_default_branch_changes_are_observed() {
    let mut repository = Repository::new();
    repository.commit("first");
    let original = repository.checkout(None, false).unwrap();
    repository.source.reference = GitReference::Tag("missing".into());
    assert!(repository.checkout(None, false).is_err());
    let children = fs::read_dir(original.root.parent().unwrap())
        .unwrap()
        .count();
    assert_eq!(children, 1);
    git(Command::new("git")
        .arg("-C")
        .arg(&repository.source.repository)
        .args(["checkout", "-b", "next"]))
    .unwrap();
    let next = repository.commit("next branch");
    repository.source.reference = GitReference::DefaultBranch;
    assert_eq!(repository.checkout(None, false).unwrap().commit, next);
}
