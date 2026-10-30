use std::{fs, path::Path, process::Command};

use scoop_pm::lockfile::LockedSource;

use super::support::*;

pub(super) fn git(path: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    if cfg!(windows) {
        command.args(["-c", "core.longpaths=true"]);
    }
    success(command.current_dir(path).args(args).output().unwrap())
        .trim()
        .into()
}

pub(super) fn init(path: &Path) {
    fs::create_dir_all(path).unwrap();
    git(path, &["init", "--initial-branch", "main"]);
    git(path, &["config", "user.name", "PM Test"]);
    git(path, &["config", "user.email", "pm@example.org"]);
}

pub(super) fn commit(path: &Path) -> String {
    git(path, &["add", "."]);
    git(path, &["commit", "-m", "test"]);
    git(path, &["rev-parse", "HEAD"])
}

fn selected(directory: &Path, name: &str) -> (String, String, Vec<String>) {
    let lock = lock(directory);
    let node = lock.resolutions[0]
        .nodes
        .iter()
        .find(|node| node.coordinate.name() == name)
        .unwrap();
    let LockedSource::Git {
        commit,
        subdir,
        references,
        ..
    } = &node.source
    else {
        panic!("expected Git source")
    };
    (commit.clone(), subdir.clone(), references.clone())
}

#[test]
fn git_workspaces_same_repo_paths_selectors_updates_and_submodules_use_exact_commits() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    #[cfg(unix)]
    {
        fs::create_dir(root.join("package-cache")).unwrap();
        std::os::unix::fs::symlink("package-cache", root.join("cache")).unwrap();
    }
    core(root);
    let assets = root.join("assets");
    init(&assets);
    write(&assets.join("data.txt"), "original data");
    commit(&assets);
    let repo = root.join("repo");
    init(&repo);
    write(
        &repo.join("Cone.toml"),
        "schema = 1\n[workspace]\nmembers=['cones/json', 'cones/util']\n[workspace.cone]\ngroup='org.example'\nversion='1.0.0'\n",
    );
    write(
        &repo.join("cones/json/Cone.toml"),
        "schema=1\n[cone]\ngroup={workspace=true}\nname='json'\nversion={workspace=true}\nkind='library'\n[dependencies]\n'org.example:util'={path='../util'}\n",
    );
    write(&repo.join("cones/util/Cone.toml"), &cone("util", "1.0.0"));
    git(
        &repo,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "../assets",
            "vendor/assets",
        ],
    );
    let first = commit(&repo);
    git(&repo, &["branch", "release"]);
    let url = url::Url::from_directory_path(&repo).unwrap();
    app(
        root,
        "app",
        &format!(
            "'org.example:json'={{git='{url}', branch='main', subdir='cones/json', version='1.0.0'}}"
        ),
    );
    success(pm(root, &root.join("app"), &["resolve"]));
    assert_eq!(
        selected(&root.join("app"), "json"),
        (
            first.clone(),
            "cones/json".into(),
            vec!["branch:main".into()]
        )
    );
    assert_eq!(
        selected(&root.join("app"), "util"),
        (first.clone(), "cones/util".into(), vec![])
    );
    let cache = scoop_pm::cache::PackageCache::resolve(root, Some(&root.join("cache"))).unwrap();
    let checkout = cache.git(url.as_str()).join("checkouts").join(&first);
    assert_eq!(
        fs::read_to_string(checkout.join("vendor/assets/data.txt")).unwrap(),
        "original data"
    );
    write(&assets.join("data.txt"), "new upstream data");
    commit(&assets);
    write(&repo.join("README.md"), "new parent commit");
    let second = commit(&repo);
    success(pm(root, &root.join("app"), &["fetch", "--offline"]));
    assert_eq!(selected(&root.join("app"), "json").0, first);
    success(pm(
        root,
        &root.join("app"),
        &["update", "--package", "org.example:json"],
    ));
    assert_eq!(selected(&root.join("app"), "json").0, second);
    assert_eq!(selected(&root.join("app"), "util").0, second);
    assert_eq!(
        fs::read_to_string(
            cache
                .git(url.as_str())
                .join("checkouts")
                .join(&second)
                .join("vendor/assets/data.txt")
        )
        .unwrap(),
        "original data"
    );
    let path = root.join("app/Cone.toml");
    let manifest = fs::read_to_string(&path).unwrap();
    let remote_manifest = repo.join("Cone.toml");
    write(
        &remote_manifest,
        &fs::read_to_string(&remote_manifest)
            .unwrap()
            .replace("version='1.0.0'", "version='2.0.0'"),
    );
    let third = commit(&repo);
    write(
        &path,
        &manifest.replace("version='1.0.0'", "version='2.0.0'"),
    );
    failure(
        pm(root, &root.join("app"), &["resolve", "--locked"]),
        "dependency resolution failed",
    );
    success(pm(root, &root.join("app"), &["resolve"]));
    assert_eq!(selected(&root.join("app"), "json").0, third);
    let manifest = fs::read_to_string(&path)
        .unwrap()
        .replace("branch='main'", "branch='release'")
        .replace("version='2.0.0'", "version='1.0.0'");
    write(&path, &manifest);
    let bytes = fs::read(root.join("app/Cone.lock")).unwrap();
    failure(
        pm(root, &root.join("app"), &["resolve", "--locked"]),
        "branch:release",
    );
    assert_eq!(fs::read(root.join("app/Cone.lock")).unwrap(), bytes);
    success(pm(root, &root.join("app"), &["resolve"]));
    assert_eq!(selected(&root.join("app"), "json").0, first);
}
