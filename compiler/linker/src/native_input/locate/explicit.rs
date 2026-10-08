use super::*;

pub(super) fn find(
    key: &NativeLinkRequirementKey,
    roots: &[PathBuf],
    profile: &ValidatedFinalLinkProfile,
) -> Result<Option<NativeFile>, LinkError> {
    let name = key.library().as_str();
    let linux = matches!(profile, ValidatedFinalLinkProfile::Linux(_));
    let dynamic = matches!(profile, ValidatedFinalLinkProfile::Linux(p) if p.mode() == scoop_toolchain::LinkMode::Dynamic);
    let suffixes = if linux {
        match key.kind() {
            NativeLibraryKind::TargetDefault => {
                let mut files = vec![
                    (format!("{name}.o"), NativeFileKind::Object),
                    (format!("lib{name}.a"), NativeFileKind::Archive),
                ];
                if dynamic {
                    files.push((format!("lib{name}.so"), NativeFileKind::SharedObject));
                }
                files
            }
            NativeLibraryKind::StaticArchive => {
                vec![(format!("lib{name}.a"), NativeFileKind::Archive)]
            }
            NativeLibraryKind::Dynamic if dynamic => {
                vec![(format!("lib{name}.so"), NativeFileKind::SharedObject)]
            }
            NativeLibraryKind::Dynamic => {
                return Err(error("ELF shared library requires --link-mode dynamic"));
            }
            NativeLibraryKind::Framework => {
                return Err(error(
                    "framework libraries are not supported by the Linux target",
                ));
            }
        }
    } else {
        match key.kind() {
            NativeLibraryKind::TargetDefault => vec![
                (format!("{name}.o"), NativeFileKind::Object),
                (format!("lib{name}.a"), NativeFileKind::Archive),
                (format!("lib{name}.dylib"), NativeFileKind::Dylib),
                (format!("lib{name}.tbd"), NativeFileKind::TextStub),
                (
                    format!("{name}.framework/{name}"),
                    NativeFileKind::Framework,
                ),
            ],
            NativeLibraryKind::StaticArchive => {
                vec![(format!("lib{name}.a"), NativeFileKind::Archive)]
            }
            NativeLibraryKind::Dynamic => vec![
                (format!("lib{name}.dylib"), NativeFileKind::Dylib),
                (format!("lib{name}.tbd"), NativeFileKind::TextStub),
            ],
            NativeLibraryKind::Framework => vec![
                (
                    format!("{name}.framework/{name}"),
                    NativeFileKind::Framework,
                ),
                (
                    format!("{name}.framework/{name}.tbd"),
                    NativeFileKind::TextStub,
                ),
            ],
        }
    };
    let paths: BTreeMap<_, _> = roots
        .iter()
        .flat_map(|root| {
            suffixes
                .iter()
                .map(move |(suffix, kind)| (root.join(suffix), *kind))
        })
        .collect();
    let mut candidates = BTreeMap::new();
    let mut locators = Vec::new();
    for (path, kind) in paths {
        match std::fs::symlink_metadata(&path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(error(format!("native candidate {}: {err}", path.display()))),
            Ok(_) => {}
        }
        let file = read(&path, kind, profile, false)
            .map_err(|err| error(format!("native candidate {}: {err}", path.display())))?;
        locators.push(path);
        if candidates
            .get(&file.id)
            .is_none_or(|previous: &NativeFile| file.locator < previous.locator)
        {
            candidates.insert(file.id, file);
        }
    }
    if candidates.is_empty() {
        return Ok(None);
    }
    if candidates.len() != 1 {
        return Err(error(format!(
            "ambiguous native library {name:?}: {locators:?}"
        )));
    }
    candidates
        .into_values()
        .next()
        .map(Some)
        .ok_or_else(|| error("native candidate disappeared"))
}
