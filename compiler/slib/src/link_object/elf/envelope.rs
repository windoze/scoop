use std::num::NonZeroU32;

use object::read::elf::{FileHeader, SectionHeader, Sym};
use object::{ObjectSection, ObjectSymbol, elf};

use super::*;
use crate::link_object::object_envelope::*;

pub fn validate_linux_elf_object_envelope_v1(
    bytes: &[u8],
    target: TargetProfileId,
) -> Result<ValidatedObjectEnvelopeV1, ElfObjectError> {
    let object = ValidatedElfObject::read(bytes, target)?;
    let file = object.file();
    let endian = file.endian();
    if file.elf_header().e_flags(endian) != 0 {
        return Err(error("nonzero flags in amd64 ELF header"));
    }
    let mut groups = Vec::new();
    let mut sections = Vec::new();
    for section in file.sections() {
        let header = section.elf_section_header();
        if section.index().0 != sections.len() + 1 {
            return Err(error("ELF section inventory is not contiguous"));
        }
        if let Some((flags, members)) = header.group(endian, bytes).map_err(error)? {
            groups.push(ObservedElfGroupV1 {
                signature_symbol: header.sh_info(endian),
                flags,
                sections: members
                    .iter()
                    .map(|s| {
                        NonZeroU32::new(s.get(endian))
                            .ok_or_else(|| error("null section group member"))
                    })
                    .collect::<Result<_, _>>()?,
            });
        }
        sections.push(ObservedObjectSectionV1 {
            segment_name: Vec::new(),
            section_name: section.name_bytes().map_err(error)?.to_vec(),
            virtual_address: section.address(),
            file_offset: section.file_range().map(|r| r.0),
            flags: ObjectSectionFlagsV1::Elf {
                section_type: header.sh_type(endian),
                flags: header.sh_flags(endian),
            },
            byte_size: section.size(),
            alignment_power: section.align().max(1).trailing_zeros(),
        });
    }
    let mut symbols = Vec::new();
    for (index, raw) in file.elf_symbol_table().enumerate() {
        // object intentionally excludes ELF's null entry from symbol_by_index.
        // Retain it here so every relocation keeps its original table index.
        if index.0 == 0 {
            if raw.st_name(endian) != 0
                || raw.st_info() != 0
                || raw.st_other() != 0
                || raw.st_shndx(endian) != 0
                || raw.st_value(endian) != 0
                || raw.st_size(endian) != 0
            {
                return Err(error("ELF null symbol entry is not zero"));
            }
            symbols.push(ObservedObjectSymbolV1 {
                table_index: 0,
                name: Vec::new(),
                kind: ObjectSymbolKindV1::FileMetadata,
                section_ordinal: None,
                value: 0,
                attributes: ObjectSymbolAttributesV1::Elf {
                    size: 0,
                    info: 0,
                    other: 0,
                },
            });
            continue;
        }
        let symbol = file.symbol_by_index(index).map_err(error)?;
        let name = symbol.name_bytes().map_err(error)?.to_vec();
        let section_ordinal = symbol
            .section_index()
            .map(|s| {
                u32::try_from(s.0).map_err(error).and_then(|s| {
                    NonZeroU32::new(s).ok_or_else(|| error("null definition section"))
                })
            })
            .transpose()?;
        let kind = classify_symbol(
            index.0,
            raw.st_info(),
            symbol.is_undefined(),
            section_ordinal,
        )?;
        symbols.push(ObservedObjectSymbolV1 {
            table_index: u32::try_from(index.0).map_err(error)?,
            name,
            kind,
            section_ordinal,
            value: symbol.address(),
            attributes: ObjectSymbolAttributesV1::Elf {
                size: symbol.size(),
                info: raw.st_info(),
                other: raw.st_other(),
            },
        });
    }
    let relocations = object
        .relocations()
        .iter()
        .map(|r| {
            let section = file.section_by_index(r.section).map_err(error)?;
            let offset = usize::try_from(r.offset).map_err(error)?;
            let mut value = [0; 8];
            let width = usize::from(r.width);
            let field = section
                .data()
                .map_err(error)?
                .get(offset..offset + width)
                .ok_or_else(|| error("ELF relocation field outside section"))?;
            value
                .get_mut(..width)
                .ok_or_else(|| error("ELF relocation field exceeds 64 bits"))?
                .copy_from_slice(field);
            Ok(ObservedObjectRelocationV1 {
                containing_section_ordinal: NonZeroU32::new(
                    u32::try_from(r.section.0).map_err(error)?,
                )
                .ok_or_else(|| error("null relocation section"))?,
                offset: r.offset,
                encoded_value: u64::from_le_bytes(value),
                shape: ObjectRelocationShapeV1::ElfRela {
                    kind: r.kind,
                    target_symbol: u32::try_from(r.symbol.0).map_err(error)?,
                    addend: r.addend,
                    width: r.width,
                },
            })
        })
        .collect::<Result<_, ElfObjectError>>()?;
    Ok(ValidatedObjectEnvelopeV1 {
        byte_length: bytes.len() as u64,
        content_digest: scoop_wire::sha256(bytes),
        format: ObjectEnvelopeFormatV1::Elf64 {
            target,
            flags: 0,
            groups,
        },
        sections,
        symbols,
        relocations,
    })
}

fn classify_symbol(
    index: usize,
    info: u8,
    undefined: bool,
    section: Option<NonZeroU32>,
) -> Result<ObjectSymbolKindV1, ElfObjectError> {
    let (binding, kind) = (info >> 4, info & 15);
    if index == 0 || (binding == elf::STB_LOCAL && kind == elf::STT_FILE) {
        return Ok(ObjectSymbolKindV1::FileMetadata);
    }
    if binding == elf::STB_LOCAL && kind == elf::STT_SECTION && section.is_some() {
        return Ok(ObjectSymbolKindV1::SectionBase);
    }
    if !matches!(
        kind,
        elf::STT_NOTYPE | elf::STT_OBJECT | elf::STT_FUNC | elf::STT_TLS
    ) {
        return Err(error(format!("unsupported builtin ELF symbol type {kind}")));
    }
    if undefined {
        return if binding == elf::STB_GLOBAL {
            Ok(ObjectSymbolKindV1::ExternalUndefined)
        } else {
            Err(error(
                "builtin ELF undefined symbol must have global binding",
            ))
        };
    }
    if section.is_none() {
        return Err(error("builtin ELF definition has no section"));
    }
    match binding {
        elf::STB_LOCAL => Ok(ObjectSymbolKindV1::LocalSectionDefinition),
        elf::STB_GLOBAL => Ok(ObjectSymbolKindV1::ExternalStrongDefinition),
        elf::STB_WEAK => Ok(ObjectSymbolKindV1::ExternalWeakDefinition),
        _ => Err(error(format!("unsupported ELF symbol binding {binding}"))),
    }
}
