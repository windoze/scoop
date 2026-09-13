//! Closed symbol forms for built-in Darwin/AArch64 object capabilities.

use std::fmt;
use std::num::NonZeroU8;

use object::read::macho::Nlist as _;
use object::{Endianness, macho};

use super::ObservedMachOSectionV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinArm64SymbolKindV1 {
    LocalSectionDefinition,
    ExternalStrongDefinition,
    ExternalUndefined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedMachOSymbolV1 {
    table_index: u32,
    name: Vec<u8>,
    kind: DarwinArm64SymbolKindV1,
    section_ordinal: Option<NonZeroU8>,
    value: u64,
    no_dead_strip: bool,
}

impl ObservedMachOSymbolV1 {
    pub const fn table_index(&self) -> u32 {
        self.table_index
    }

    pub fn name(&self) -> &[u8] {
        &self.name
    }

    pub const fn kind(&self) -> DarwinArm64SymbolKindV1 {
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
}

pub(super) fn validate_darwin_arm64_symbol_inventory_v1(
    symtab: &macho::SymtabCommand<Endianness>,
    dysymtab: &macho::DysymtabCommand<Endianness>,
    endian: Endianness,
    bytes: &[u8],
    sections: &[ObservedMachOSectionV1],
) -> Result<Vec<ObservedMachOSymbolV1>, DarwinArm64SymbolInventoryValidationError> {
    let table = symtab
        .symbols::<macho::MachHeader64<Endianness>, _>(endian, bytes)
        .map_err(|_| DarwinArm64SymbolInventoryValidationError::MalformedSymbolTable)?;
    let mut symbols = Vec::with_capacity(table.len());
    for (index, symbol) in table.iter().enumerate() {
        let table_index = u32::try_from(index)
            .map_err(|_| DarwinArm64SymbolInventoryValidationError::MalformedSymbolTable)?;
        let name = symbol.name(endian, table.strings()).map_err(|_| {
            DarwinArm64SymbolInventoryValidationError::InvalidSymbolName { index: table_index }
        })?;
        if name.is_empty() {
            return Err(
                DarwinArm64SymbolInventoryValidationError::InvalidSymbolName { index: table_index },
            );
        }
        symbols.push(classify_symbol(
            table_index,
            name,
            symbol,
            endian,
            sections,
        )?);
    }
    validate_partition(dysymtab, endian, &symbols)?;
    Ok(symbols)
}

fn classify_symbol(
    table_index: u32,
    name: &[u8],
    symbol: &macho::Nlist64<Endianness>,
    endian: Endianness,
    sections: &[ObservedMachOSectionV1],
) -> Result<ObservedMachOSymbolV1, DarwinArm64SymbolInventoryValidationError> {
    let symbol_type = symbol.n_type;
    let description = symbol.n_desc.get(endian);
    let value = symbol.n_value.get(endian);
    let (kind, section_ordinal, no_dead_strip) = match symbol_type {
        macho::N_SECT => (
            DarwinArm64SymbolKindV1::LocalSectionDefinition,
            validate_definition_location(table_index, symbol.n_sect, value, sections)?,
            validate_definition_description(table_index, description)?,
        ),
        symbol_type if symbol_type == (macho::N_SECT | macho::N_EXT) => (
            DarwinArm64SymbolKindV1::ExternalStrongDefinition,
            validate_definition_location(table_index, symbol.n_sect, value, sections)?,
            validate_definition_description(table_index, description)?,
        ),
        symbol_type if symbol_type == (macho::N_UNDF | macho::N_EXT) => {
            if symbol.n_sect != macho::NO_SECT || value != 0 || description != 0 {
                return Err(
                    DarwinArm64SymbolInventoryValidationError::InvalidUndefinedSymbol {
                        index: table_index,
                    },
                );
            }
            (DarwinArm64SymbolKindV1::ExternalUndefined, None, false)
        }
        _ => {
            return Err(
                DarwinArm64SymbolInventoryValidationError::UnsupportedSymbolType {
                    index: table_index,
                    actual: symbol_type,
                },
            );
        }
    };
    Ok(ObservedMachOSymbolV1 {
        table_index,
        name: name.to_vec(),
        kind,
        section_ordinal,
        value,
        no_dead_strip,
    })
}

fn validate_definition_location(
    index: u32,
    ordinal: u8,
    value: u64,
    sections: &[ObservedMachOSectionV1],
) -> Result<Option<NonZeroU8>, DarwinArm64SymbolInventoryValidationError> {
    let ordinal = NonZeroU8::new(ordinal).ok_or(
        DarwinArm64SymbolInventoryValidationError::InvalidDefinitionSection {
            index,
            section_ordinal: ordinal,
        },
    )?;
    let section = sections.get(usize::from(ordinal.get()) - 1).ok_or(
        DarwinArm64SymbolInventoryValidationError::InvalidDefinitionSection {
            index,
            section_ordinal: ordinal.get(),
        },
    )?;
    let end = section
        .virtual_address()
        .checked_add(section.byte_size())
        .ok_or(DarwinArm64SymbolInventoryValidationError::DefinitionValueOutOfBounds { index })?;
    if value < section.virtual_address() || value > end {
        return Err(
            DarwinArm64SymbolInventoryValidationError::DefinitionValueOutOfBounds { index },
        );
    }
    Ok(Some(ordinal))
}

fn validate_definition_description(
    index: u32,
    description: u16,
) -> Result<bool, DarwinArm64SymbolInventoryValidationError> {
    match description {
        0 => Ok(false),
        macho::N_NO_DEAD_STRIP => Ok(true),
        actual => Err(
            DarwinArm64SymbolInventoryValidationError::UnsupportedSymbolDescription {
                index,
                actual,
            },
        ),
    }
}

fn validate_partition(
    record: &macho::DysymtabCommand<Endianness>,
    endian: Endianness,
    symbols: &[ObservedMachOSymbolV1],
) -> Result<(), DarwinArm64SymbolInventoryValidationError> {
    let local_end = record.nlocalsym.get(endian);
    let external_end = local_end
        .checked_add(record.nextdefsym.get(endian))
        .ok_or(DarwinArm64SymbolInventoryValidationError::MalformedSymbolTable)?;
    for symbol in symbols {
        let expected = if symbol.table_index < local_end {
            DarwinArm64SymbolKindV1::LocalSectionDefinition
        } else if symbol.table_index < external_end {
            DarwinArm64SymbolKindV1::ExternalStrongDefinition
        } else {
            DarwinArm64SymbolKindV1::ExternalUndefined
        };
        if symbol.kind != expected {
            return Err(
                DarwinArm64SymbolInventoryValidationError::DynamicPartitionKindMismatch {
                    index: symbol.table_index,
                    expected,
                    actual: symbol.kind,
                },
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinArm64SymbolInventoryValidationError {
    MalformedSymbolTable,
    InvalidSymbolName {
        index: u32,
    },
    UnsupportedSymbolType {
        index: u32,
        actual: u8,
    },
    InvalidDefinitionSection {
        index: u32,
        section_ordinal: u8,
    },
    DefinitionValueOutOfBounds {
        index: u32,
    },
    UnsupportedSymbolDescription {
        index: u32,
        actual: u16,
    },
    InvalidUndefinedSymbol {
        index: u32,
    },
    DynamicPartitionKindMismatch {
        index: u32,
        expected: DarwinArm64SymbolKindV1,
        actual: DarwinArm64SymbolKindV1,
    },
}

impl fmt::Display for DarwinArm64SymbolInventoryValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Darwin/AArch64 object symbol inventory: {self:?}"
        )
    }
}

impl std::error::Error for DarwinArm64SymbolInventoryValidationError {}

#[cfg(test)]
mod tests;
