use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use scoop_manifest::package::{GitReference, GitSource};
use scoop_process::CommandExt;

use crate::{PmError, PmResult, cache::PackageCache};

mod command;
use command::{git, object_exists, resolve_commit};

#[derive(Clone, Debug)]
pub struct GitCheckout {
    pub repository: String,
    pub commit: String,
    pub root: PathBuf,
}

/// The caller supplies a locked commit for normal builds. An unpinned request
/// is a first resolution or explicit update; offline requests use local refs.
pub fn checkout(
    cache: &PackageCache,
    source: &GitSource,
    base: &Path,
    locked: Option<&str>,
    offline: bool,
) -> PmResult<GitCheckout> {
    let repository = repository_url(&source.repository, base)?;
    let directory = cache.git(&repository);
    let checkouts = directory.join("checkouts");
    let cached = |commit: &str| GitCheckout {
        repository: repository.clone(),
        commit: commit.to_owned(),
        root: checkouts.join(commit),
    };
    if let Some(commit) = locked {
        validate_commit(commit)?;
        if cached(commit)
            .root
            .try_exists()
            .map_err(|error| PmError::io(&checkouts, error))?
        {
            return Ok(cached(commit));
        }
    }
    let _guard = cache.lock("git", repository.as_bytes())?;
    if let Some(commit) = locked {
        if cached(commit)
            .root
            .try_exists()
            .map_err(|error| PmError::io(&checkouts, error))?
        {
            return Ok(cached(commit));
        }
    }
    let db = directory.join("db");
    let exists = db.try_exists().map_err(|error| PmError::io(&db, error))?;
    if !exists {
        if offline {
            return Err(PmError::new(format!(
                "Git objects for {repository} are not cached; fetch without --offline"
            )));
        }
        fs::create_dir_all(&directory).map_err(|error| PmError::io(&directory, error))?;
        let staged =
            tempfile::tempdir_in(&directory).map_err(|error| PmError::io(&directory, error))?;
        git(Command::new("git")
            .arg("clone")
            .arg("--mirror")
            .arg("--")
            .arg(&repository)
            .arg(staged.path()))?;
        fs::rename(staged.path(), &db).map_err(|error| PmError::io(&db, error))?;
    } else if locked.is_none() && !offline {
        git(Command::new("git")
            .arg("--git-dir")
            .arg(&db)
            .args(["fetch", "--prune", "origin"]))?;
        if source.reference == GitReference::DefaultBranch {
            command::refresh_head(&db)?;
        }
    }
    let commit = if let Some(commit) = locked {
        if !object_exists(&db, commit)? {
            if offline {
                return Err(PmError::new(format!(
                    "Git commit {commit} for {repository} is not cached; fetch without --offline"
                )));
            }
            git(Command::new("git")
                .arg("--git-dir")
                .arg(&db)
                .args(["fetch", "origin", commit]))?;
        }
        let full = resolve_commit(&db, commit)?;
        if full != commit {
            return Err(PmError::new(
                "Cone.lock must contain the full Git commit ID",
            ));
        }
        full
    } else {
        let reference = match &source.reference {
            GitReference::DefaultBranch => "HEAD".into(),
            GitReference::Branch(branch) => format!("refs/heads/{branch}"),
            GitReference::Tag(tag) => format!("refs/tags/{tag}"),
            GitReference::Revision(revision) => revision.clone(),
        };
        resolve_commit(&db, &reference)?
    };
    let result = cached(&commit);
    if result
        .root
        .try_exists()
        .map_err(|error| PmError::io(&result.root, error))?
    {
        return Ok(result);
    }
    materialize(&db, &result, offline)?;
    Ok(result)
}

fn materialize(db: &Path, checkout: &GitCheckout, offline: bool) -> PmResult<()> {
    let tree = git(Command::new("git").arg("--git-dir").arg(db).args([
        "ls-tree",
        "-r",
        &checkout.commit,
    ]))?;
    let submodules = tree.lines().any(|line| line.starts_with("160000 "));
    if offline && submodules {
        return Err(PmError::new(format!(
            "checkout {} needs uncached submodules; fetch without --offline",
            checkout.commit
        )));
    }
    let directory = checkout.root.parent().expect("checkout path has a parent");
    fs::create_dir_all(directory).map_err(|error| PmError::io(directory, error))?;
    let staged = tempfile::tempdir_in(directory).map_err(|error| PmError::io(directory, error))?;
    git(Command::new("git")
        .args(["clone", "--shared", "--no-checkout", "--"])
        .arg(db)
        .arg(staged.path()))?;
    git(Command::new("git").arg("-C").arg(staged.path()).args([
        "remote",
        "set-url",
        "origin",
        &checkout.repository,
    ]))?;
    git(Command::new("git").arg("-C").arg(staged.path()).args([
        "checkout",
        "--detach",
        &checkout.commit,
    ]))?;
    if submodules {
        git(Command::new("git").arg("-C").arg(staged.path()).args([
            "submodule",
            "update",
            "--init",
            "--recursive",
        ]))?;
    }
    fs::rename(staged.path(), &checkout.root)
        .map_err(|error| PmError::io(&checkout.root, error))?;
    Ok(())
}

pub fn repository_url(repository: &str, base: &Path) -> PmResult<String> {
    if repository.is_empty() || repository.contains('\0') {
        return Err(PmError::new(
            "Git repository must not be empty or contain NUL",
        ));
    }
    if repository.contains("://") {
        return url::Url::parse(repository)
            .map(String::from)
            .map_err(|error| PmError::new(format!("invalid Git repository URL: {error}")));
    }
    if repository.contains(':') && !Path::new(repository).is_absolute() {
        return Ok(repository.to_owned()); // Preserve system Git's SCP-like syntax.
    }
    let path = base.join(repository);
    let path = fs::canonicalize(&path).map_err(|error| PmError::io(&path, error))?;
    url::Url::from_directory_path(&path)
        .map(String::from)
        .map_err(|()| {
            PmError::new(format!(
                "cannot represent Git repository {} as a file URL",
                path.display()
            ))
        })
}

fn validate_commit(commit: &str) -> PmResult<()> {
    if commit.is_empty()
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Err(PmError::new(
            "locked Git commit must be a full lowercase hexadecimal object ID",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
