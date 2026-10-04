use super::*;

pub(super) struct Commands<'a> {
    pub name: String,
    pub current: u32,
    pub compatibility: u32,
    pub dependencies: Vec<LoadDependency>,
    pub rpaths: Vec<String>,
    pub trie: &'a [u8],
    pub base: u64,
}

pub(super) fn read<'a>(
    file: &MachOFile64<'a>,
    bytes: &'a [u8],
    profile: &ValidatedFinalLinkProfile,
) -> Result<Commands<'a>, LinkError> {
    let endian = file.endian();
    let mut commands = file.macho_load_commands().map_err(error)?;
    let mut identity = None;
    let mut trie = None;
    let mut dependencies = Vec::new();
    let mut rpaths = Vec::new();
    let mut builds = 0;
    let mut base = None;
    while let Some(command) = commands.next().map_err(error)? {
        match command.cmd() {
            macho::LC_ID_DYLIB | macho::LC_LOAD_DYLIB | macho::LC_REEXPORT_DYLIB => {
                let dylib: &macho::DylibCommand<object::Endianness> =
                    command.data().map_err(error)?;
                let name =
                    std::str::from_utf8(command.string(endian, dylib.dylib.name).map_err(error)?)
                        .map_err(error)?
                        .to_owned();
                if command.cmd() == macho::LC_ID_DYLIB {
                    if identity
                        .replace((
                            name,
                            dylib.dylib.current_version.get(endian),
                            dylib.dylib.compatibility_version.get(endian),
                        ))
                        .is_some()
                    {
                        return Err(error("duplicate native LC_ID_DYLIB"));
                    }
                } else {
                    dependencies.push(LoadDependency {
                        name,
                        reexport: command.cmd() == macho::LC_REEXPORT_DYLIB,
                        compatibility_version: dylib.dylib.compatibility_version.get(endian),
                    });
                }
            }
            macho::LC_RPATH => {
                let path: &macho::RpathCommand<object::Endianness> =
                    command.data().map_err(error)?;
                rpaths.push(
                    std::str::from_utf8(command.string(endian, path.path).map_err(error)?)
                        .map_err(error)?
                        .to_owned(),
                );
            }
            macho::LC_BUILD_VERSION => {
                let build = command
                    .build_version()
                    .map_err(error)?
                    .ok_or_else(|| error("invalid native dylib build version"))?;
                if build.platform.get(endian) != macho::PLATFORM_MACOS
                    || build.minos.get(endian)
                        > profile
                            .startup_toolchain()
                            .profile()
                            .contract()
                            .deployment()
                            .map_err(error)?
                            .minimum_os()
                            .packed()
                {
                    return Err(error("native dylib has incompatible platform/deployment"));
                }
                builds += 1;
            }
            macho::LC_DYLD_EXPORTS_TRIE => {
                let data: &macho::LinkeditDataCommand<object::Endianness> =
                    command.data().map_err(error)?;
                if trie
                    .replace(range(
                        bytes,
                        data.dataoff.get(endian),
                        data.datasize.get(endian),
                    )?)
                    .is_some()
                {
                    return Err(error("duplicate native export trie"));
                }
            }
            macho::LC_DYLD_INFO_ONLY | macho::LC_DYLD_INFO => {
                let info = command
                    .dyld_info()
                    .map_err(error)?
                    .ok_or_else(|| error("invalid native dyld info"))?;
                if info.export_size.get(endian) != 0
                    && trie
                        .replace(range(
                            bytes,
                            info.export_off.get(endian),
                            info.export_size.get(endian),
                        )?)
                        .is_some()
                {
                    return Err(error("duplicate native export trie"));
                }
                if info.weak_bind_size.get(endian) != 0 {
                    return Err(error("native dylib requires cross-provider weak lookup"));
                }
            }
            macho::LC_SEGMENT_64 => {
                let (segment, _) = command
                    .segment_64()
                    .map_err(error)?
                    .ok_or_else(|| error("invalid native dylib segment"))?;
                let offset = segment.fileoff.get(endian);
                let size = segment.filesize.get(endian);
                if offset
                    .checked_add(size)
                    .is_none_or(|end| end > bytes.len() as u64)
                    || size > segment.vmsize.get(endian)
                    || segment
                        .vmaddr
                        .get(endian)
                        .checked_add(segment.vmsize.get(endian))
                        .is_none()
                {
                    return Err(error("native dylib segment exceeds file/VM range"));
                }
                if offset == 0 && size != 0 {
                    base = Some(segment.vmaddr.get(endian));
                }
            }
            macho::LC_LOAD_WEAK_DYLIB
            | macho::LC_LOAD_UPWARD_DYLIB
            | macho::LC_LAZY_LOAD_DYLIB
            | macho::LC_DYLD_ENVIRONMENT
            | macho::LC_LINKER_OPTION => {
                return Err(error(format!(
                    "native dylib uses unsupported load-command semantics {:#x}",
                    command.cmd()
                )));
            }
            _ => continue,
        }
    }
    let (name, current, compatibility) =
        identity.ok_or_else(|| error("native dylib has no LC_ID_DYLIB"))?;
    if builds != 1 {
        return Err(error("native dylib requires one LC_BUILD_VERSION"));
    }
    Ok(Commands {
        name,
        current,
        compatibility,
        dependencies,
        rpaths,
        trie: trie.unwrap_or(&[]),
        base: base.ok_or_else(|| error("native dylib header has no segment"))?,
    })
}

fn range(bytes: &[u8], offset: u32, length: u32) -> Result<&[u8], LinkError> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| error("native export range overflow"))?;
    bytes
        .get(offset as usize..end as usize)
        .ok_or_else(|| error("native export range exceeds file"))
}
