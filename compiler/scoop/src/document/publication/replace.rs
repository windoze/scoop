use std::{fs, io, path::Path};

pub(super) fn replace(stage: &Path, output: &Path) -> io::Result<()> {
    if output.try_exists()? {
        replace_existing(stage, output)
    } else {
        fs::rename(stage, output)
    }
}

#[cfg(unix)]
fn replace_existing(stage: &Path, output: &Path) -> io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        stage,
        rustix::fs::CWD,
        output,
        rustix::fs::RenameFlags::EXCHANGE,
    )
    .map_err(Into::into)
}

#[cfg(windows)]
fn replace_existing(stage: &Path, output: &Path) -> io::Result<()> {
    let parent = output
        .parent()
        .ok_or_else(|| io::Error::other("site directory has no parent"))?;
    let backup = tempfile::Builder::new()
        .prefix(".scoop-doc-previous-")
        .tempdir_in(parent)?;
    fs::remove_dir(backup.path())?;
    fs::rename(output, backup.path())?;
    if let Err(error) = fs::rename(stage, output) {
        if let Err(restore) = fs::rename(backup.path(), output) {
            let previous = backup.keep();
            return Err(io::Error::other(format!(
                "cannot publish site: {error}; cannot restore previous site: {restore}; previous site remains at {}",
                previous.display()
            )));
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_new_and_replaces_existing_directories() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("site");
        for content in ["first", "second"] {
            let stage = tempfile::tempdir_in(root.path()).unwrap();
            fs::write(stage.path().join("index.html"), content).unwrap();
            replace(stage.path(), &output).unwrap();
            assert_eq!(
                fs::read_to_string(output.join("index.html")).unwrap(),
                content
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn locked_new_site_restores_the_previous_directory() {
        use std::os::windows::fs::OpenOptionsExt;

        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("site");
        fs::create_dir(&output).unwrap();
        fs::write(output.join("index.html"), "previous").unwrap();
        let stage = tempfile::tempdir_in(root.path()).unwrap();
        let page = stage.path().join("index.html");
        fs::write(&page, "new").unwrap();
        let held = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&page)
            .unwrap();
        assert!(replace(stage.path(), &output).is_err());
        assert_eq!(
            fs::read_to_string(output.join("index.html")).unwrap(),
            "previous"
        );
        drop(held);
        assert_eq!(fs::read_to_string(page).unwrap(), "new");
        replace(stage.path(), &output).unwrap();
        assert_eq!(
            fs::read_to_string(output.join("index.html")).unwrap(),
            "new"
        );
    }
}
