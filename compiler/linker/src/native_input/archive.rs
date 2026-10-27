use super::*;
use object::read::archive::{ArchiveFile, ArchiveKind};

pub(crate) struct Member {
    pub id: NativeArchiveMemberId,
    pub name: String,
    pub range: Range<usize>,
    pub digest: Digest256,
    pub index: MemberIndex,
}

pub(crate) enum MemberIndex {
    Object {
        info: NativeObjectInfo,
        imports: bool,
    },
    Import(coff_import::CoffImport),
}

impl Member {
    pub fn object_index(&self) -> Option<&NativeObjectInfo> {
        match &self.index {
            MemberIndex::Object { info, .. } => Some(info),
            MemberIndex::Import(_) => None,
        }
    }
    pub fn import(&self) -> Option<&coff_import::CoffImport> {
        match &self.index {
            MemberIndex::Import(import) => Some(import),
            MemberIndex::Object { .. } => None,
        }
    }
    pub fn has_imports(&self) -> bool {
        match self.index {
            MemberIndex::Import(_) => true,
            MemberIndex::Object { imports, .. } => imports,
        }
    }
}

pub(super) fn read(
    bytes: &[u8],
    input: NativeInputId,
    slice: &Range<usize>,
    profile: &ValidatedFinalLinkProfile,
) -> Result<Vec<Member>, LinkError> {
    let data = &bytes[slice.clone()];
    let archive = ArchiveFile::parse(data).map_err(error)?;
    if archive.is_thin() || archive.kind() == ArchiveKind::AixBig {
        return Err(error(
            "native archive must be a normal BSD/GNU/COFF archive, not thin or external-member",
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
        let index = if profile.target().native_object_format()
            == scoop_lir::NativeObjectFormat::Coff64
        {
            if object::FileKind::parse(payload).ok() == Some(object::FileKind::CoffImport) {
                coff_import::CoffImport::read(payload).map(MemberIndex::Import)
            } else {
                crate::native_object::coff::read_with_import_sections(payload)
                    .map(|(info, imports)| MemberIndex::Object { info, imports })
            }
        } else {
            NativeObjectInfo::read_with_toolchain(payload, profile.startup_toolchain().profile())
                .map(|info| MemberIndex::Object {
                    info,
                    imports: false,
                })
        }
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
