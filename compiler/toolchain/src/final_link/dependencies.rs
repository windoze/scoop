//! Header inputs of the actual link-tool capability probe.
use super::*;
use std::collections::BTreeSet;

pub(super) fn read(
    depfile: &Path,
    source: &Path,
    stderr: &[u8],
) -> Result<Vec<PathBuf>, ToolchainError> {
    let text = std::fs::read_to_string(depfile).map_err(error)?;
    let mut paths: BTreeSet<_> = scoop_process::parse_make_dependencies(&text, "link-probe")
        .ok_or_else(|| error("link probe returned an invalid dependency list"))?
        .into_iter()
        .filter(|path| path != source)
        .collect();
    crate::native_record::header_candidates(stderr, &mut paths);
    Ok(paths.into_iter().collect())
}
