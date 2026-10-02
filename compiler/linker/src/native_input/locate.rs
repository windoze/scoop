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
            format!("{name}.o"),
            format!("lib{name}.a"),
            format!("lib{name}.dylib"),
            format!("lib{name}.tbd"),
            format!("{name}.framework/{name}"),
        ],
        NativeLibraryKind::StaticArchive => vec![format!("lib{name}.a")],
        NativeLibraryKind::Dynamic => vec![format!("lib{name}.dylib"), format!("lib{name}.tbd")],
        NativeLibraryKind::Framework => vec![format!("{name}.framework/{name}")],
    };
    let paths: std::collections::BTreeSet<_> = roots
        .iter()
        .flat_map(|root| suffixes.iter().map(move |suffix| root.join(suffix)))
        .collect();
    let mut candidates = BTreeMap::new();
    let mut locators = Vec::new();
    for path in paths {
        match std::fs::symlink_metadata(&path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(error(format!("native candidate {}: {err}", path.display()))),
            Ok(_) => {}
        }
        let file = read(&path, profile)
            .map_err(|err| error(format!("native candidate {}: {err}", path.display())))?;
        locators.push(path);
        candidates.entry(file.id).or_insert(file);
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

fn read(path: &Path, profile: &ValidatedFinalLinkProfile) -> Result<NativeFile, LinkError> {
    let bytes = std::fs::read(path).map_err(error)?;
    let slice = slice::select(&bytes)?;
    let archive = path.extension().is_some_and(|extension| extension == "a");
    let id = NativeInputId(
        domain_separated_cbor_hash(
            "scoop-native-input-v1",
            &FileKey {
                profile,
                bytes: &bytes,
                kind: if archive { 2 } else { 1 },
            },
        )
        .map_err(error)?,
    );
    let content = if archive {
        NativeContent::Archive(archive::read(&bytes, id, &slice, profile)?)
    } else {
        let index = NativeObjectIndex::read(
            &bytes[slice.clone()],
            profile
                .startup_toolchain()
                .profile()
                .contract()
                .deployment(),
        )?;
        index.check_selected(&bytes[slice.clone()])?;
        NativeContent::Object(index)
    };
    Ok(NativeFile {
        id,
        bytes: bytes.into(),
        slice,
        content,
        locator: std::fs::canonicalize(path).map_err(error)?,
    })
}
