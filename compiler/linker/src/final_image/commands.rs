use super::*;

impl FinalImage<'_> {
    pub(super) fn read_commands(
        &mut self,
        profile: &ValidatedFinalLinkProfile,
        inputs: &ProgramInputs<'_>,
    ) -> Result<(), LinkError> {
        let namespace = inputs.namespace.darwin()?;
        let endian = self.file.endian();
        let mut commands = self.file.macho_load_commands().map_err(error)?;
        let mut entry = None;
        let mut build = None;
        let mut dyld = None;
        let mut signature = None;
        let mut libraries = BTreeSet::new();
        let mut rpaths = BTreeSet::new();
        let mut interpreters = 0;
        while let Some(command) = commands.next().map_err(error)? {
            match command.cmd() {
                macho::LC_SEGMENT_64 => {
                    let (segment, _) = command
                        .segment_64()
                        .map_err(error)?
                        .ok_or_else(|| error("invalid final segment"))?;
                    let file_size = segment.filesize.get(endian);
                    let offset = segment.fileoff.get(endian);
                    if offset
                        .checked_add(file_size)
                        .is_none_or(|end| end > self.bytes.len() as u64)
                        || file_size > segment.vmsize.get(endian)
                        || segment
                            .vmaddr
                            .get(endian)
                            .checked_add(segment.vmsize.get(endian))
                            .is_none()
                    {
                        return Err(error("final segment exceeds file/VM range"));
                    }
                    self.segments.push(Segment {
                        address: segment.vmaddr.get(endian),
                        size: segment.vmsize.get(endian),
                        offset,
                        file_size,
                    });
                }
                macho::LC_MAIN => {
                    if entry
                        .replace(
                            command
                                .entry_point()
                                .map_err(error)?
                                .ok_or_else(|| error("invalid LC_MAIN"))?
                                .entryoff
                                .get(endian),
                        )
                        .is_some()
                    {
                        return Err(error("multiple final LC_MAIN commands"));
                    }
                }
                macho::LC_BUILD_VERSION => {
                    if build
                        .replace(
                            command
                                .build_version()
                                .map_err(error)?
                                .ok_or_else(|| error("invalid final build version"))?,
                        )
                        .is_some()
                    {
                        return Err(error("multiple final build versions"));
                    }
                }
                macho::LC_DYLD_INFO_ONLY => {
                    if dyld
                        .replace(
                            command
                                .dyld_info()
                                .map_err(error)?
                                .ok_or_else(|| error("invalid final dyld info"))?,
                        )
                        .is_some()
                    {
                        return Err(error("multiple final dyld info commands"));
                    }
                }
                macho::LC_LOAD_DYLIB => {
                    let library = command
                        .dylib()
                        .map_err(error)?
                        .ok_or_else(|| error("invalid final library"))?;
                    let name = command.string(endian, library.dylib.name).map_err(error)?;
                    let provider = namespace
                        .providers
                        .stubs
                        .keys()
                        .filter_map(|id| namespace.providers.providers.get(id))
                        .find(|provider| provider.install_name.as_bytes() == name)
                        .ok_or_else(|| {
                            error(format!(
                                "unexpected final dynamic provider {}",
                                String::from_utf8_lossy(name)
                            ))
                        })?;
                    if library.dylib.current_version.get(endian) != provider.current_version
                        || library.dylib.compatibility_version.get(endian)
                            != provider.compatibility_version
                    {
                        return Err(error(format!(
                            "final dynamic provider {} version differs from its input",
                            provider.install_name
                        )));
                    }
                    if !libraries.insert(provider.id) {
                        return Err(error("duplicate final dynamic provider load command"));
                    }
                    self.providers.push(provider.id);
                }
                macho::LC_RPATH => {
                    let path: &macho::RpathCommand<object::Endianness> =
                        command.data().map_err(error)?;
                    let value =
                        std::str::from_utf8(command.string(endian, path.path).map_err(error)?)
                            .map_err(error)?
                            .to_owned();
                    if !namespace.providers.rpaths.contains(&value) || !rpaths.insert(value.clone())
                    {
                        return Err(error(format!(
                            "unexpected or duplicate final RPATH {value}"
                        )));
                    }
                }
                macho::LC_LOAD_DYLINKER => {
                    let interpreter: &macho::DylinkerCommand<object::Endianness> =
                        command.data().map_err(error)?;
                    if command.string(endian, interpreter.name).map_err(error)? != b"/usr/lib/dyld"
                    {
                        return Err(error("unexpected final dynamic loader"));
                    }
                    interpreters += 1;
                }
                macho::LC_CODE_SIGNATURE => {
                    let data: &macho::LinkeditDataCommand<object::Endianness> =
                        command.data().map_err(error)?;
                    if signature
                        .replace((data.dataoff.get(endian), data.datasize.get(endian)))
                        .is_some()
                    {
                        return Err(error("multiple final code signatures"));
                    }
                }
                macho::LC_DYLD_CHAINED_FIXUPS
                | macho::LC_LOAD_WEAK_DYLIB
                | macho::LC_REEXPORT_DYLIB
                | macho::LC_LOAD_UPWARD_DYLIB
                | macho::LC_DYLD_ENVIRONMENT
                | macho::LC_DYLD_EXPORTS_TRIE => {
                    return Err(error(
                        "final output uses a fixup or provider outside the selected profile",
                    ));
                }
                _ => continue,
            }
        }
        let build = build.ok_or_else(|| error("final output has no deployment command"))?;
        let expected = profile
            .startup_toolchain()
            .profile()
            .contract()
            .deployment()
            .map_err(error)?;
        if build.platform.get(endian) != macho::PLATFORM_MACOS
            || build.minos.get(endian) != expected.minimum_os().packed()
            || build.sdk.get(endian) != expected.sdk().packed()
            || libraries != namespace.providers.stubs.keys().copied().collect()
            || rpaths != namespace.providers.rpaths
            || self.file.macho_header().flags.get(endian) & macho::MH_TWOLEVEL == 0
            || interpreters != 1
            || self.file.macho_header().flags.get(endian) & macho::MH_PIE == 0
        {
            return Err(error(
                "final deployment, PIE or system provider differs from the selected profile",
            ));
        }
        let entry = entry.ok_or_else(|| error("final output has no LC_MAIN"))?;
        let main = self.symbol("_main")?;
        let mapped = self.segments.iter().find_map(|segment| {
            entry
                .checked_sub(segment.offset)
                .filter(|offset| *offset < segment.file_size)
                .and_then(|offset| segment.address.checked_add(offset))
        });
        if mapped != Some(main) {
            return Err(error("LC_MAIN does not point at the program C main"));
        }
        let info = dyld.ok_or_else(|| error("final output has no classic dyld info"))?;
        let mut ranges: Vec<_> = self
            .segments
            .iter()
            .filter(|segment| segment.size != 0)
            .map(|segment| (segment.address, segment.address + segment.size))
            .collect();
        ranges.sort_unstable();
        if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
            return Err(error("final VM segments overlap"));
        }
        let base = self
            .segments
            .iter()
            .find(|segment| segment.offset == 0 && segment.file_size != 0)
            .ok_or_else(|| error("final executable header has no VM mapping"))?
            .address;
        self.exports = exports::read(
            self.file_range(info.export_off.get(endian), info.export_size.get(endian))?,
            base,
        )?;
        let rebase = self.file_range(info.rebase_off.get(endian), info.rebase_size.get(endian))?;
        self.rebases = fixups::rebases(rebase, &self.segments)?;
        for (offset, length) in [
            (info.bind_off.get(endian), info.bind_size.get(endian)),
            (
                info.lazy_bind_off.get(endian),
                info.lazy_bind_size.get(endian),
            ),
        ] {
            for (address, binding) in
                fixups::bindings(self.file_range(offset, length)?, &self.segments)?
            {
                if self.bindings.insert(address, binding).is_some() {
                    return Err(error("duplicate final pointer binding"));
                }
            }
        }
        self.weak_bindings = fixups::bindings(
            self.file_range(
                info.weak_bind_off.get(endian),
                info.weak_bind_size.get(endian),
            )?,
            &self.segments,
        )?;
        let (offset, length) =
            signature.ok_or_else(|| error("final output has no ad-hoc code signature"))?;
        signature::check(self.file_range(offset, length)?, offset)?;
        Ok(())
    }

    fn file_range(&self, offset: u32, length: u32) -> Result<&[u8], LinkError> {
        let end = offset
            .checked_add(length)
            .ok_or_else(|| error("final load-command range overflow"))?;
        self.bytes
            .get(offset as usize..end as usize)
            .ok_or_else(|| error("final load-command range exceeds file"))
    }
}
