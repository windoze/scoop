//! LLVM literal pools are local object data referenced by machine code.

use std::num::NonZeroU32;

use super::{
    BuiltinObjectSectionRoleV1, DarwinArm64RelocationShapeV1, DarwinArm64RelocationTargetV1,
    ObjectRelocationShapeV1, ObjectSymbolKindV1, ObservedObjectSectionV1,
    StrongObjectDefinitionValidationError as Error, ValidatedBuiltinObjectSectionInventoryV1,
    VerifiedStrongObjectDefinitionV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ObjectLiteral {
    Bytes4([u8; 4]),
    Bytes8([u8; 8]),
    Bytes16([u8; 16]),
    ByteSpan(Vec<u8>),
}

impl ObjectLiteral {
    pub(super) fn bytes(&self) -> &[u8] {
        match self {
            Self::Bytes4(bytes) => bytes,
            Self::Bytes8(bytes) => bytes,
            Self::Bytes16(bytes) => bytes,
            Self::ByteSpan(bytes) => bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LiteralPool {
    section: NonZeroU32,
    address: u64,
    width: usize,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LiteralPools(Vec<LiteralPool>);

pub(super) fn literal_width(section: &ObservedObjectSectionV1) -> Option<usize> {
    match (section.segment_name(), section.section_name()) {
        (b"__TEXT", b"__literal4") | (b"", b".rodata.cst4") => Some(4),
        (b"__TEXT", b"__literal8") | (b"", b".rodata.cst8") => Some(8),
        (b"__TEXT", b"__literal16") | (b"", b".rodata.cst16") => Some(16),
        _ => None,
    }
}

impl LiteralPools {
    pub(super) fn read(
        object: &[u8],
        sections: &ValidatedBuiltinObjectSectionInventoryV1,
        definitions: &[VerifiedStrongObjectDefinitionV1],
    ) -> Result<Self, Error> {
        let mut pools = Vec::new();
        for (index, section) in sections.envelope().sections().iter().enumerate() {
            let ordinal = u32::try_from(index + 1)
                .ok()
                .and_then(NonZeroU32::new)
                .ok_or(Error::UnsupportedSectionOrdinal { index: u32::MAX })?;
            let invalid = || Error::InvalidSectionByteRange { section: ordinal };
            let width = if let Some(width) = literal_width(section) {
                width
            } else if is_local_table(sections, ordinal, definitions) {
                usize::try_from(section.byte_size()).map_err(|_| invalid())?
            } else {
                continue;
            };
            let start = section.file_offset().ok_or_else(invalid)?;
            let end = start.checked_add(section.byte_size()).ok_or_else(invalid)?;
            let bytes = object
                .get(
                    usize::try_from(start).map_err(|_| invalid())?
                        ..usize::try_from(end).map_err(|_| invalid())?,
                )
                .ok_or_else(invalid)?;
            if bytes.len() % width != 0 {
                return Err(invalid());
            }
            pools.push(LiteralPool {
                section: ordinal,
                address: section.virtual_address(),
                width,
                bytes: bytes.to_vec(),
            });
        }
        Ok(Self(pools))
    }

    pub(super) fn contains_section(&self, section: NonZeroU32) -> bool {
        self.0.iter().any(|pool| pool.section == section)
    }

    pub(super) fn at(&self, section: NonZeroU32, address: u64) -> Option<ObjectLiteral> {
        let pool = self.0.iter().find(|pool| pool.section == section)?;
        let offset = usize::try_from(address.checked_sub(pool.address)?).ok()?;
        if offset % pool.width != 0 {
            return None;
        }
        let bytes = pool.bytes.get(offset..offset.checked_add(pool.width)?)?;
        match pool.width {
            4 => Some(ObjectLiteral::Bytes4(bytes.try_into().ok()?)),
            8 => Some(ObjectLiteral::Bytes8(bytes.try_into().ok()?)),
            16 => Some(ObjectLiteral::Bytes16(bytes.try_into().ok()?)),
            _ => Some(ObjectLiteral::ByteSpan(bytes.to_vec())),
        }
    }
}

fn is_local_table(
    sections: &ValidatedBuiltinObjectSectionInventoryV1,
    ordinal: NonZeroU32,
    definitions: &[VerifiedStrongObjectDefinitionV1],
) -> bool {
    let envelope = sections.envelope();
    let section = &envelope.sections()[ordinal.get() as usize - 1];
    if section.segment_name() != b"__TEXT"
        || section.section_name() != b"__const"
        || section.byte_size() == 0
        || definitions
            .iter()
            .flat_map(|definition| definition.atoms())
            .any(|atom| atom.section_ordinal() == ordinal)
        || envelope
            .relocations()
            .iter()
            .any(|relocation| relocation.containing_section_ordinal() == ordinal)
    {
        return false;
    }
    envelope.relocations().iter().any(|relocation| {
        if sections.roles()[relocation.containing_section_ordinal().get() as usize - 1]
            != BuiltinObjectSectionRoleV1::Text
        {
            return false;
        }
        let ObjectRelocationShapeV1::DarwinArm64(
            DarwinArm64RelocationShapeV1::Page21 { target, .. }
            | DarwinArm64RelocationShapeV1::PageOffset12 { target, .. },
        ) = relocation.shape()
        else {
            return false;
        };
        let DarwinArm64RelocationTargetV1::SymbolTableIndex(index) = target else {
            return false;
        };
        envelope.symbols().iter().any(|symbol| {
            symbol.table_index() == index
                && symbol.kind() == ObjectSymbolKindV1::LocalSectionDefinition
                && symbol.section_ordinal() == Some(ordinal)
                && symbol.value() == section.virtual_address()
        })
    })
}

#[cfg(test)]
mod tests;
