//! LLVM literal pools are local object data referenced by machine code.

use std::num::NonZeroU32;

use super::{
    ObjectDefinitionReadError as Error, ObjectDefinitionV1, ObservedObjectSectionV1,
    ValidatedBuiltinObjectSectionInventoryV1,
};

mod tables;

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
        definitions: &[ObjectDefinitionV1],
    ) -> Result<Self, Error> {
        let mut pools = Vec::new();
        for (index, section) in sections.envelope().sections().iter().enumerate() {
            let ordinal = u32::try_from(index + 1)
                .ok()
                .and_then(NonZeroU32::new)
                .ok_or(Error::UnsupportedSectionOrdinal { index: u32::MAX })?;
            if let Some(width) = literal_width(section) {
                pools.push(read_pool(
                    object,
                    section,
                    ordinal,
                    section.virtual_address(),
                    width,
                    section.byte_size(),
                )?);
            } else {
                let atoms = definitions
                    .iter()
                    .flat_map(|definition| definition.atoms())
                    .filter(|atom| atom.section_ordinal() == ordinal)
                    .map(|atom| atom.start()..atom.end())
                    .collect::<Vec<_>>();
                for range in tables::ranges(sections, ordinal, &atoms) {
                    let size = range.end - range.start;
                    let width = usize::try_from(size)
                        .map_err(|_| Error::InvalidSectionByteRange { section: ordinal })?;
                    pools.push(read_pool(
                        object,
                        section,
                        ordinal,
                        range.start,
                        width,
                        size,
                    )?);
                }
            }
        }
        Ok(Self(pools))
    }

    pub(super) fn at(&self, section: NonZeroU32, address: u64) -> Option<ObjectLiteral> {
        self.0
            .iter()
            .filter(|pool| pool.section == section)
            .find_map(|pool| {
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
            })
    }
}

fn read_pool(
    object: &[u8],
    section: &ObservedObjectSectionV1,
    ordinal: NonZeroU32,
    address: u64,
    width: usize,
    byte_size: u64,
) -> Result<LiteralPool, Error> {
    let invalid = || Error::InvalidSectionByteRange { section: ordinal };
    let offset = address
        .checked_sub(section.virtual_address())
        .ok_or_else(invalid)?;
    let start = section
        .file_offset()
        .and_then(|start| start.checked_add(offset))
        .ok_or_else(invalid)?;
    let end = start.checked_add(byte_size).ok_or_else(invalid)?;
    let bytes = object
        .get(
            usize::try_from(start).map_err(|_| invalid())?
                ..usize::try_from(end).map_err(|_| invalid())?,
        )
        .ok_or_else(invalid)?;
    if bytes.len() % width != 0 {
        return Err(invalid());
    }
    Ok(LiteralPool {
        section: ordinal,
        address,
        width,
        bytes: bytes.to_vec(),
    })
}

#[cfg(test)]
mod tests;
