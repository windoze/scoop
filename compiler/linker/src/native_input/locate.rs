use super::*;
use scoop_identity::NativeLibraryKind;

mod explicit;
mod system;

pub(super) struct LibraryResolution {
    pub files: Vec<NativeFile>,
    pub scripts: Vec<(PathBuf, Digest256)>,
    pub system_alias: bool,
}

pub(super) fn library(
    key: &NativeLinkRequirementKey,
    roots: &[PathBuf],
    profile: &ValidatedFinalLinkProfile,
) -> Result<LibraryResolution, LinkError> {
    match explicit::find(key, roots, profile)? {
        Some(file) => Ok(LibraryResolution {
            files: vec![file],
            scripts: Vec::new(),
            system_alias: false,
        }),
        None => system::find(key, profile),
    }
}

pub(crate) fn read(
    path: &Path,
    kind: NativeFileKind,
    profile: &ValidatedFinalLinkProfile,
    system: bool,
) -> Result<NativeFile, LinkError> {
    let bytes = std::fs::read(path).map_err(error)?;
    read_bytes(
        std::fs::canonicalize(path).map_err(error)?,
        bytes,
        kind,
        profile,
        system,
    )
}

fn read_bytes(
    locator: PathBuf,
    bytes: Vec<u8>,
    kind: NativeFileKind,
    profile: &ValidatedFinalLinkProfile,
    system: bool,
) -> Result<NativeFile, LinkError> {
    let slice = if kind == NativeFileKind::TextStub
        || (kind == NativeFileKind::Archive && bytes.starts_with(b"!<arch>\n"))
    {
        0..bytes.len()
    } else {
        slice::select(&bytes)?
    };
    let id = NativeInputId::from_bytes(&bytes, kind, profile)?;
    let fixed_system = matches!(kind, NativeFileKind::Archive | NativeFileKind::SharedObject)
        && matches!(profile, ValidatedFinalLinkProfile::Linux(linux) if linux
            .input_paths()
            .any(|path| path.canonicalize().ok().as_ref() == Some(&locator)));
    let content = match kind {
        _ if fixed_system => NativeContent::System,
        NativeFileKind::Archive => {
            NativeContent::Archive(archive::read(&bytes, id, &slice, profile)?)
        }
        NativeFileKind::Object => {
            let index = NativeObjectIndex::read_with_toolchain(
                &bytes[slice.clone()],
                profile.startup_toolchain().profile(),
                profile.cxx(),
            )?;
            NativeContent::Object(index)
        }
        NativeFileKind::SharedObject => NativeContent::ElfDynamic(Arc::new(
            elf_dynamic::ElfDynamic::read(&bytes[slice.clone()], profile.id())?,
        )),
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
