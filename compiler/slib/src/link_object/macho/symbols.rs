//! Read symbol names, locations and binding attributes for Darwin/AArch64 objects.

use std::fmt;
use std::num::NonZeroU8;

use object::read::macho::Nlist as _;
use object::{Endianness, macho};

use super::ObservedMachOSectionV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachOSymbolKindV1 {
    FileMetadata,
    LocalSectionDefinition,
    ExternalStrongDefinition,
    ExternalWeakDefinition,
    ExternalUndefined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedMachOSymbolV1 {
    table_index: u32,
    name: Vec<u8>,
    kind: MachOSymbolKindV1,
    section_ordinal: Option<NonZeroU8>,
    value: u64,
    no_dead_strip: bool,
    private_external: bool,
}

impl ObservedMachOSymbolV1 {
    pub const fn table_index(&self) -> u32 {
        self.table_index
    }

    pub fn name(&self) -> &[u8] {
        &self.name
    }

    pub const fn kind(&self) -> MachOSymbolKindV1 {
        self.kind
    }

    pub const fn section_ordinal(&self) -> Option<NonZeroU8> {
        self.section_ordinal
    }

    pub const fn value(&self) -> u64 {
        self.value
    }

    pub const fn no_dead_strip(&self) -> bool {
        self.no_dead_strip
    }

    pub const fn private_external(&self) -> bool {
        self.private_external
    }
}

pub(super) fn validate_macho_symbol_inventory_v1(
    symtab: &macho::SymtabCommand<Endianness>,
    endian: Endianness,
    bytes: &[u8],
    sections: &[ObservedMachOSectionV1],
) -> Result<Vec<ObservedMachOSymbolV1>, MachOSymbolInventoryValidationError> {
    let table = symtab
        .symbols::<macho::MachHeader64<Endianness>, _>(endian, bytes)
        .map_err(|_| MachOSymbolInventoryValidationError::MalformedSymbolTable)?;
    let mut symbols = Vec::with_capacity(table.len());
    for (index, symbol) in table.iter().enumerate() {
        let table_index = u32::try_from(index)
            .map_err(|_| MachOSymbolInventoryValidationError::MalformedSymbolTable)?;
        let name = symbol.name(endian, table.strings()).map_err(|_| {
            MachOSymbolInventoryValidationError::InvalidSymbolName { index: table_index }
        })?;
        symbols.push(classify_symbol(
            table_index,
            name,
            symbol,
            endian,
            sections,
        )?);
    }
    Ok(symbols)
}

fn classify_symbol(
    table_index: u32,
    name: &[u8],
    symbol: &macho::Nlist64<Endianness>,
    endian: Endianness,
    sections: &[ObservedMachOSectionV1],
) -> Result<ObservedMachOSymbolV1, MachOSymbolInventoryValidationError> {
    let symbol_type = symbol.n_type;
    let description = symbol.n_desc.get(endian);
    let value = symbol.n_value.get(endian);
    let (kind, section_ordinal) = if symbol_type & macho::N_STAB != 0 {
        (MachOSymbolKindV1::FileMetadata, None)
    } else {
        match symbol_type & macho::N_TYPE {
            macho::N_SECT => (
                if symbol_type & macho::N_EXT != 0 {
                    external_definition_kind(description)
                } else {
                    MachOSymbolKindV1::LocalSectionDefinition
                },
                validate_definition_location(table_index, symbol.n_sect, value, sections)?,
            ),
            macho::N_UNDF if value == 0 => (MachOSymbolKindV1::ExternalUndefined, None),
            macho::N_UNDF | macho::N_ABS => (external_definition_kind(description), None),
            _ => (MachOSymbolKindV1::FileMetadata, None),
        }
    };
    Ok(ObservedMachOSymbolV1 {
        table_index,
        name: name.to_vec(),
        kind,
        section_ordinal,
        value,
        no_dead_strip: description & macho::N_NO_DEAD_STRIP != 0,
        private_external: symbol_type & macho::N_PEXT != 0,
    })
}

fn validate_definition_location(
    index: u32,
    ordinal: u8,
    value: u64,
    sections: &[ObservedMachOSectionV1],
) -> Result<Option<NonZeroU8>, MachOSymbolInventoryValidationError> {
    let ordinal = NonZeroU8::new(ordinal).ok_or(
        MachOSymbolInventoryValidationError::InvalidDefinitionSection {
            index,
            section_ordinal: ordinal,
        },
    )?;
    let section = sections.get(usize::from(ordinal.get()) - 1).ok_or(
        MachOSymbolInventoryValidationError::InvalidDefinitionSection {
            index,
            section_ordinal: ordinal.get(),
        },
    )?;
    let end = section
        .virtual_address()
        .checked_add(section.byte_size())
        .ok_or(MachOSymbolInventoryValidationError::DefinitionValueOutOfBounds { index })?;
    if value < section.virtual_address() || value > end {
        return Err(MachOSymbolInventoryValidationError::DefinitionValueOutOfBounds { index });
    }
    Ok(Some(ordinal))
}

fn external_definition_kind(description: u16) -> MachOSymbolKindV1 {
    if description & macho::N_WEAK_DEF != 0 {
        MachOSymbolKindV1::ExternalWeakDefinition
    } else {
        MachOSymbolKindV1::ExternalStrongDefinition
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachOSymbolInventoryValidationError {
    MalformedSymbolTable,
    InvalidSymbolName { index: u32 },
    UnsupportedSymbolType { index: u32, actual: u8 },
    InvalidDefinitionSection { index: u32, section_ordinal: u8 },
    DefinitionValueOutOfBounds { index: u32 },
    UnsupportedSymbolDescription { index: u32, actual: u16 },
    InvalidUndefinedSymbol { index: u32 },
}

impl fmt::Display for MachOSymbolInventoryValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Mach-O object symbol inventory: {self:?}"
        )
    }
}

impl std::error::Error for MachOSymbolInventoryValidationError {}

#[cfg(test)]
mod tests;
