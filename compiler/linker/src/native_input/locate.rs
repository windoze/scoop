use super::*;
use scoop_identity::NativeLibraryKind;

pub(super) fn library(
    key: &NativeLinkRequirementKey,
    roots: &[PathBuf],
    profile: &ValidatedFinalLinkProfile,
) -> Result<NativeFile, LinkError> {
    let name = key.library().as_str();
    let suffixes = match key.kind() {
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
        NativeLibraryKind::StaticArchive => vec![(format!("lib{name}.a"), NativeFileKind::Archive)],
        NativeLibraryKind::Dynamic => vec![
            (format!("lib{name}.dylib"), NativeFileKind::Dylib),
            (format!("lib{name}.tbd"), NativeFileKind::TextStub),
        ],
        NativeLibraryKind::Framework => vec![(
            format!("{name}.framework/{name}"),
            NativeFileKind::Framework,
        )],
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
        return Err(error(format!(
            "missing native library {name:?}; searched explicit roots {roots:?}"
        )));
    }
    if candidates.len() != 1 {
        return Err(error(format!(
            "ambiguous native library {name:?}: {locators:?}"
        )));
    }
    candidates
        .into_values()
        .next()
        .ok_or_else(|| error("native candidate disappeared"))
}

pub(crate) fn read(
    path: &Path,
    kind: NativeFileKind,
    profile: &ValidatedFinalLinkProfile,
    system: bool,
) -> Result<NativeFile, LinkError> {
    let bytes = std::fs::read(path).map_err(error)?;
    let slice = if kind == NativeFileKind::TextStub {
        0..bytes.len()
    } else {
        slice::select(&bytes)?
    };
    let id = NativeInputId::from_bytes(&bytes, kind, profile)?;
    let locator = std::fs::canonicalize(path).map_err(error)?;
    let content = match kind {
        NativeFileKind::Archive => {
            NativeContent::Archive(archive::read(&bytes, id, &slice, profile)?)
        }
        NativeFileKind::Object => {
            let index = NativeObjectIndex::read(
                &bytes[slice.clone()],
                profile
                    .startup_toolchain()
                    .profile()
                    .contract()
                    .deployment(),
            )?;
            NativeContent::Object(index)
        }
        NativeFileKind::Dylib | NativeFileKind::TextStub | NativeFileKind::Framework => {
            let records = crate::dynamic::read(
                &bytes[slice.clone()],
                id,
                &locator,
                profile,
                kind == NativeFileKind::TextStub,
                system,
            )?;
            if records.is_empty() {
                return Err(error("native dynamic input has no install-name records"));
            }
            NativeContent::Dynamic(records)
        }
    };
    Ok(NativeFile {
        id,
        bytes: bytes.into(),
        slice,
        content,
        locator,
    })
}
