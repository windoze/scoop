use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub(super) fn publish_map(map: &scoop_profile::ProfileMap, artifact: &Path) -> io::Result<PathBuf> {
    let destination = scoop_toolchain::profile_map_path(artifact);
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::other("profile map has no parent"))?;
    std::fs::create_dir_all(parent)?;
    // tempfile's rename uses the Windows API directly, which needs the extended path.
    let parent = std::fs::canonicalize(parent)?;
    let destination = parent.join(destination.file_name().expect("profile map basename"));
    let mut output = tempfile::NamedTempFile::new_in(&parent)?;
    serde_json::to_writer(output.as_file_mut(), map)?;
    output.write_all(b"\n")?;
    output.persist(&destination).map_err(|error| error.error)?;
    Ok(destination)
}
