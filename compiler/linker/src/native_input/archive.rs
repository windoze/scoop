use super::*;
use object::read::archive::{ArchiveFile, ArchiveKind};

pub(crate) struct Member {
    pub id: NativeArchiveMemberId,
    pub name: String,
    pub range: Range<usize>,
    pub digest: Digest256,
    pub index: NativeObjectIndex,
}

pub(super) fn read(
    bytes: &[u8],
    input: NativeInputId,
    slice: &Range<usize>,
    profile: &ValidatedFinalLinkProfile,
) -> Result<Vec<Member>, LinkError> {
    let data = &bytes[slice.clone()];
    let archive = ArchiveFile::parse(data).map_err(error)?;
    if archive.is_thin() || matches!(archive.kind(), ArchiveKind::Coff | ArchiveKind::AixBig) {
        return Err(error(
            "native archive must be a normal BSD/GNU archive, not thin or external-member",
        ));
    }
    let headers = headers(data)?;
    let mut members = Vec::new();
    let mut offsets = BTreeMap::new();
    for member in archive.members() {
        let member = member.map_err(error)?;
        let header = member
            .header()
            .ok_or_else(|| error("native archive has no normal member header"))?;
        let header_offset = (header as *const _ as usize)
            .checked_sub(data.as_ptr() as usize)
            .ok_or_else(|| error("native archive header is outside its input"))?;
        let ordinal = *headers
            .get(&header_offset)
            .ok_or_else(|| error("native archive member has an invalid physical header offset"))?;
        let (offset, length) = member.file_range();
        let payload = member.data(data).map_err(error)?;
        let name = std::str::from_utf8(member.name())
            .map_err(error)?
            .to_owned();
        if name.is_empty()
            || name.starts_with("__.SYMDEF")
            || matches!(name.as_str(), "/" | "//" | "/SYM64/")
        {
            return Err(error(format!(
                "native archive has an unknown or misplaced special member {name:?}"
            )));
        }
        if payload.starts_with(&object::archive::MAGIC)
            || payload.starts_with(&object::archive::THIN_MAGIC)
        {
            return Err(error(format!(
                "native archive member {ordinal} ({name}) is a nested archive"
            )));
        }
        let index = NativeObjectIndex::read(
            payload,
            profile
                .startup_toolchain()
                .profile()
                .contract()
                .deployment()
                .map_err(error)?,
        )
        .map_err(|err| error(format!("archive member {ordinal} ({name}): {err}")))?;
        let start = slice
            .start
            .checked_add(usize::try_from(offset).map_err(error)?)
            .ok_or_else(|| error("archive payload offset overflow"))?;
        let end = start
            .checked_add(payload.len())
            .ok_or_else(|| error("archive payload end overflow"))?;
        let id = NativeArchiveMemberId {
            input,
            ordinal,
            header_offset: (slice.start + header_offset) as u64,
            payload_offset: start as u64,
            payload_length: length,
        };
        offsets.insert(header_offset as u64, members.len());
        members.push(Member {
            id,
            name,
            range: start..end,
            digest: sha256(payload),
            index,
        });
    }
    if let Some(symbols) = archive.symbols().map_err(error)? {
        for symbol in symbols {
            let symbol = symbol.map_err(error)?;
            let member = offsets
                .get(&symbol.offset().0)
                .map(|index| &members[*index])
                .ok_or_else(|| {
                    error("archive symbol table offset does not point to a physical member")
                })?;
            let name = std::str::from_utf8(symbol.name()).map_err(error)?;
            if !member.index.info.definitions.contains_key(name) {
                return Err(error(format!(
                    "archive symbol table names {name} absent from member {} ({})",
                    member.id.ordinal, member.name
                )));
            }
        }
    }
    Ok(members)
}

// ArchiveFile handles names and TOCs; this walk supplies physical ordinals and
// checks the full container span, including skipped special-member payloads.
fn headers(bytes: &[u8]) -> Result<BTreeMap<usize, u64>, LinkError> {
    let mut result = BTreeMap::new();
    let mut offset = object::archive::MAGIC.len();
    while offset < bytes.len() {
        let header_end = offset
            .checked_add(60)
            .ok_or_else(|| error("archive header overflow"))?;
        let header = bytes
            .get(offset..header_end)
            .ok_or_else(|| error("archive member header exceeds file"))?;
        if &header[58..] != b"`\n" {
            return Err(error("invalid archive member terminator"));
        }
        let length: usize = std::str::from_utf8(&header[48..58])
            .map_err(error)?
            .trim_end()
            .parse()
            .map_err(error)?;
        let end = header_end
            .checked_add(length)
            .and_then(|end| end.checked_add(length & 1))
            .ok_or_else(|| error("archive member range overflow"))?;
        if end > bytes.len() {
            return Err(error("archive member payload/padding exceeds file"));
        }
        result.insert(offset, result.len() as u64);
        offset = end;
    }
    Ok(result)
}
