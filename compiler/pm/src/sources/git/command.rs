use super::*;

pub(super) fn git(command: &mut Command) -> PmResult<String> {
    let output = command
        .scoop_output()
        .map_err(|error| PmError::new(format!("cannot run Git: {error}")))?;
    if !output.status.success() {
        return Err(PmError::new(format!(
            "Git exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| PmError::new(format!("Git returned invalid UTF-8: {error}")))
}

pub(super) fn resolve_commit(db: &Path, reference: &str) -> PmResult<String> {
    let commit = git(Command::new("git").arg("--git-dir").arg(db).args([
        "rev-parse",
        "--verify",
        "--end-of-options",
        &format!("{reference}^{{commit}}"),
    ]))?;
    let commit = commit.trim().to_owned();
    validate_commit(&commit)?;
    Ok(commit)
}

pub(super) fn object_exists(db: &Path, commit: &str) -> PmResult<bool> {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(db)
        .args(["cat-file", "-e", &format!("{commit}^{{commit}}")])
        .scoop_output()
        .map_err(|error| PmError::new(format!("cannot inspect Git object {commit}: {error}")))?;
    Ok(output.status.success())
}

pub(super) fn refresh_head(db: &Path) -> PmResult<()> {
    let response = git(Command::new("git").arg("--git-dir").arg(db).args([
        "ls-remote",
        "--symref",
        "origin",
        "HEAD",
    ]))?;
    for line in response.lines() {
        if let Some(reference) = line
            .strip_prefix("ref: ")
            .and_then(|line| line.strip_suffix("\tHEAD"))
        {
            git(Command::new("git").arg("--git-dir").arg(db).args([
                "symbolic-ref",
                "HEAD",
                reference,
            ]))?;
            return Ok(());
        }
    }
    let commit = response
        .lines()
        .find_map(|line| line.strip_suffix("\tHEAD"))
        .ok_or_else(|| PmError::new("Git remote has no default HEAD commit"))?;
    validate_commit(commit)?;
    git(Command::new("git").arg("--git-dir").arg(db).args([
        "update-ref",
        "--no-deref",
        "HEAD",
        commit,
    ]))?;
    Ok(())
}
