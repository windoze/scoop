//! AMD64 COFF bounds, symbols and relocations, including the bigobj container.

use std::num::NonZeroU32;

use object::read::coff::{CoffBigFile, CoffFile, CoffHeader};
use object::{Architecture, FileKind, Object, ObjectKind, ObjectSection, SectionIndex};
use scoop_identity::TargetProfileId;

use super::object_envelope::*;

pub(crate) mod builtin;
mod relocations;
mod symbols;
#[cfg(all(test, target_os = "windows"))]
mod tests;

pub fn validate_windows_coff_object_envelope_v1(
    bytes: &[u8],
    target: TargetProfileId,
) -> Result<ValidatedObjectEnvelopeV1, CoffObjectError> {
    if !matches!(
        target,
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID
            | scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID
            | scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID
    ) {
        return Err(error("expected a registered Windows x64 target"));
    }
    match FileKind::parse(bytes).map_err(error)? {
        FileKind::Coff => envelope(
            bytes,
            &CoffFile::<&[u8]>::parse(bytes).map_err(error)?,
            target,
        ),
        FileKind::CoffBig => envelope(bytes, &CoffBigFile::parse(bytes).map_err(error)?, target),
        _ => Err(error("expected a COFF relocatable object or bigobj")),
    }
}

fn envelope<'data, H: CoffHeader>(
    bytes: &[u8],
    file: &CoffFile<'data, &'data [u8], H>,
    target: TargetProfileId,
) -> Result<ValidatedObjectEnvelopeV1, CoffObjectError> {
    if file.architecture() != Architecture::X86_64 || file.kind() != ObjectKind::Relocatable {
        return Err(error("expected an AMD64 COFF relocatable object"));
    }
    let mut sections = Vec::new();
    for section in file.sections() {
        // ObjectSection::relocations suppresses malformed-table errors. Read
        // the raw table in relocations::read instead, preserving I/O failures.
        section.data().map_err(error)?;
        sections.push(ObservedObjectSectionV1 {
            segment_name: Vec::new(),
            section_name: section.name_bytes().map_err(error)?.to_vec(),
            virtual_address: section.address(),
            file_offset: section.file_range().map(|range| range.0),
            flags: ObjectSectionFlagsV1::Coff {
                characteristics: section
                    .coff_section()
                    .characteristics
                    .get(object::LittleEndian),
            },
            byte_size: section.size(),
            alignment_power: section.align().max(1).trailing_zeros(),
        });
    }
    let symbols = symbols::read(file, &sections)?;
    let relocations = relocations::read(file, &symbols)?;
    Ok(ValidatedObjectEnvelopeV1 {
        byte_length: bytes.len() as u64,
        content_digest: scoop_wire::sha256(bytes),
        format: ObjectEnvelopeFormatV1::CoffAmd64 { target },
        sections,
        symbols,
        relocations,
    })
}

fn ordinal(index: SectionIndex) -> Result<NonZeroU32, CoffObjectError> {
    NonZeroU32::new(u32::try_from(index.0).map_err(error)?)
        .ok_or_else(|| error("zero COFF section number"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoffObjectError(String);

impl std::fmt::Display for CoffObjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "COFF object: {}", self.0)
    }
}
impl std::error::Error for CoffObjectError {}

fn error(message: impl std::fmt::Display) -> CoffObjectError {
    CoffObjectError(message.to_string())
}
