use super::*;

pub(super) fn suffixes(
    key: &NativeLinkRequirementKey,
) -> Result<Vec<(String, NativeFileKind)>, LinkError> {
    let name = key.library().as_str();
    let archives = [
        (format!("{name}.lib"), NativeFileKind::Archive),
        (format!("lib{name}.a"), NativeFileKind::Archive),
        (format!("lib{name}.dll.a"), NativeFileKind::Archive),
    ];
    match key.kind() {
        NativeLibraryKind::TargetDefault => Ok([
            (format!("{name}.obj"), NativeFileKind::Object),
            (format!("{name}.o"), NativeFileKind::Object),
        ]
        .into_iter()
        .chain(archives)
        .collect()),
        NativeLibraryKind::StaticArchive | NativeLibraryKind::Dynamic => Ok(archives.into()),
        NativeLibraryKind::Framework => Err(error(
            "framework libraries are not supported by the Windows target",
        )),
    }
}

pub(super) fn matches_kind(file: &NativeFile, kind: NativeLibraryKind) -> bool {
    let NativeContent::Archive(members) = &file.content else {
        return true;
    };
    let imports = members.iter().any(|member| member.has_imports());
    match kind {
        NativeLibraryKind::StaticArchive => !imports,
        NativeLibraryKind::Dynamic => imports,
        _ => true,
    }
}

pub(super) fn system(
    key: &NativeLinkRequirementKey,
    profile: &ValidatedFinalLinkProfile,
    windows: &scoop_toolchain::WindowsFinalLinkProfile,
    observed: &mut Vec<PathBuf>,
) -> Result<LibraryResolution, LinkError> {
    let suffixes = suffixes(key)?;
    for directory in windows.library_search_paths() {
        for (suffix, kind) in &suffixes {
            let path = directory.join(suffix);
            observed.push(path.clone());
            if !path.try_exists().map_err(error)? {
                continue;
            }
            let file = read(&path, *kind, profile, true)?;
            if matches_kind(&file, key.kind()) {
                return Ok(LibraryResolution {
                    files: vec![file],
                    scripts: Vec::new(),
                    system_alias: false,
                });
            }
        }
    }
    Err(error(format!(
        "native library {:?} is unavailable for {}",
        key.library().as_str(),
        profile.canonical_triple()
    )))
}
