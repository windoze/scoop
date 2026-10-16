use super::*;
use object::read::archive::{ArchiveFile, ArchiveKind};

pub(crate) struct Member {
    pub id: NativeArchiveMemberId,
    pub name: String,
    pub range: Range<usize>,
    pub digest: Digest256,
    pub index: NativeObjectInfo,
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
    let mut members = Vec::new();
    for (ordinal, member) in archive.members().enumerate() {
        let ordinal = ordinal as u64;
        let member = member.map_err(error)?;
        let header = member
            .header()
            .ok_or_else(|| error("native archive has no normal member header"))?;
        let header_offset = (header as *const _ as usize)
            .checked_sub(data.as_ptr() as usize)
            .ok_or_else(|| error("native archive header is outside its input"))?;
        let (offset, length) = member.file_range();
        let payload = member.data(data).map_err(error)?;
        let name = std::str::from_utf8(member.name())
            .map_err(error)?
            .to_owned();
        if payload.starts_with(&object::archive::MAGIC)
            || payload.starts_with(&object::archive::THIN_MAGIC)
        {
            return Err(error(format!(
                "native archive member {ordinal} ({name}) is a nested archive"
            )));
        }
        let index =
            NativeObjectInfo::read_with_toolchain(payload, profile.startup_toolchain().profile())
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
        members.push(Member {
            id,
            name,
            range: start..end,
            digest: sha256(payload),
            index,
        });
    }
    Ok(members)
}
