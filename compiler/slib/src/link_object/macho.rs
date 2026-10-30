//! Read the Mach-O sections, symbols and relocations used by object production.

use std::fmt;

use object::read::macho::MachHeader as _;
use object::read::macho::{Section as _, Segment as _};
use object::{Endianness, macho};
use scoop_wire::{Digest256, sha256};

mod architecture;
pub use architecture::MachOArchitecture;

mod sections;
pub use sections::*;

mod symbols;
pub use symbols::*;

mod relocations;
pub use relocations::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedMachOObjectEnvelopeV1 {
    target: scoop_identity::TargetProfileId,
    byte_length: u64,
    content_digest: Digest256,
    section_count: u32,
    symbol_count: u32,
    relocation_count: u64,
    sections: Vec<ObservedMachOSectionV1>,
    symbols: Vec<ObservedMachOSymbolV1>,
    relocations: Vec<ObservedMachORelocationV1>,
}

impl ValidatedMachOObjectEnvelopeV1 {
    pub const fn target(&self) -> scoop_identity::TargetProfileId {
        self.target
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub const fn content_digest(&self) -> Digest256 {
        self.content_digest
    }

    pub const fn section_count(&self) -> u32 {
        self.section_count
    }

    pub const fn symbol_count(&self) -> u32 {
        self.symbol_count
    }

    pub const fn relocation_count(&self) -> u64 {
        self.relocation_count
    }

    pub fn sections(&self) -> &[ObservedMachOSectionV1] {
        &self.sections
    }

    pub fn symbols(&self) -> &[ObservedMachOSymbolV1] {
        &self.symbols
    }

    pub fn relocations(&self) -> &[ObservedMachORelocationV1] {
        &self.relocations
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedMachOSectionV1 {
    segment_name: [u8; 16],
    section_name: [u8; 16],
    virtual_address: u64,
    file_offset: Option<u64>,
    relocation_file_offset: u64,
    flags: u32,
    byte_size: u64,
    alignment_power: u32,
    relocation_count: u32,
}

impl ObservedMachOSectionV1 {
    pub fn segment_name(&self) -> &[u8] {
        trimmed_fixed_name(&self.segment_name)
    }

    pub fn section_name(&self) -> &[u8] {
        trimmed_fixed_name(&self.section_name)
    }

    pub const fn flags(self) -> u32 {
        self.flags
    }

    pub const fn virtual_address(self) -> u64 {
        self.virtual_address
    }

    pub(crate) const fn file_offset(self) -> Option<u64> {
        self.file_offset
    }

    pub(super) const fn relocation_file_offset(self) -> u64 {
        self.relocation_file_offset
    }

    pub const fn byte_size(self) -> u64 {
        self.byte_size
    }

    pub const fn alignment_power(self) -> u32 {
        self.alignment_power
    }

    pub const fn relocation_count(self) -> u32 {
        self.relocation_count
    }
}

fn trimmed_fixed_name(name: &[u8; 16]) -> &[u8] {
    &name[..name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(name.len())]
}

pub fn validate_macho_object_envelope_v1(
    target: scoop_identity::TargetProfileId,
    bytes: &[u8],
) -> Result<ValidatedMachOObjectEnvelopeV1, ObjectEnvelopeValidationError> {
    let architecture = MachOArchitecture::for_target(target)?;
    let byte_length =
        u64::try_from(bytes.len()).map_err(|_| ObjectEnvelopeValidationError::FileRangeOverflow)?;

    let header = macho::MachHeader64::<Endianness>::parse(bytes, 0)
        .map_err(|_| ObjectEnvelopeValidationError::MalformedHeader)?;
    if !header.is_little_endian() || header.magic() != macho::MH_CIGAM_64 {
        return Err(ObjectEnvelopeValidationError::WrongEncoding);
    }
    let endian = header
        .endian()
        .map_err(|_| ObjectEnvelopeValidationError::WrongEncoding)?;
    if header.cputype(endian) != architecture.cpu_type() {
        return Err(ObjectEnvelopeValidationError::WrongCpuType(
            header.cputype(endian),
        ));
    }
    if header.cpusubtype(endian) != architecture.cpu_subtype() {
        return Err(ObjectEnvelopeValidationError::WrongCpuSubtype(
            header.cpusubtype(endian),
        ));
    }
    if header.filetype(endian) != macho::MH_OBJECT {
        return Err(ObjectEnvelopeValidationError::WrongFileType(
            header.filetype(endian),
        ));
    }
    let mut commands = header
        .load_commands(endian, bytes, 0)
        .map_err(|_| ObjectEnvelopeValidationError::MalformedLoadCommands)?;
    let mut observed_sections = Vec::new();
    let mut symtab = None;
    while let Some(command) = commands
        .next()
        .map_err(|_| ObjectEnvelopeValidationError::MalformedLoadCommands)?
    {
        match command.cmd() {
            macho::LC_SEGMENT_64 => {
                let (record, section_bytes) = command
                    .segment_64()
                    .map_err(|_| ObjectEnvelopeValidationError::MalformedSegment)?
                    .ok_or(ObjectEnvelopeValidationError::MalformedSegment)?;
                for section in record
                    .sections(endian, section_bytes)
                    .map_err(|_| ObjectEnvelopeValidationError::MalformedSegment)?
                {
                    observed_sections.push(ObservedMachOSectionV1 {
                        segment_name: section.segname,
                        section_name: section.sectname,
                        virtual_address: section.addr.get(endian),
                        file_offset: section.file_range(endian).map(|(offset, _)| offset),
                        relocation_file_offset: u64::from(section.reloff.get(endian)),
                        flags: section.flags.get(endian),
                        byte_size: section.size.get(endian),
                        alignment_power: section.align.get(endian),
                        relocation_count: section.nreloc.get(endian),
                    });
                }
            }
            macho::LC_SYMTAB => {
                symtab = command
                    .symtab()
                    .map_err(|_| ObjectEnvelopeValidationError::MalformedSymbolTable)?;
            }
            _ => {}
        }
    }
    let symtab = symtab.ok_or(ObjectEnvelopeValidationError::MissingSymbolTable)?;
    let symbol_count = symtab.nsyms.get(endian);
    let symbols = validate_macho_symbol_inventory_v1(symtab, endian, bytes, &observed_sections)
        .map_err(ObjectEnvelopeValidationError::SymbolInventory)?;
    let relocations = validate_macho_relocation_inventory_v1(
        architecture,
        bytes,
        &observed_sections,
        symbol_count,
    )
    .map_err(ObjectEnvelopeValidationError::RelocationInventory)?;
    Ok(ValidatedMachOObjectEnvelopeV1 {
        target,
        byte_length,
        content_digest: sha256(bytes),
        section_count: observed_sections.len() as u32,
        symbol_count,
        relocation_count: observed_sections
            .iter()
            .map(|s| u64::from(s.relocation_count()))
            .sum(),
        sections: observed_sections,
        symbols,
        relocations,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectEnvelopeValidationError {
    UnsupportedTarget,
    MalformedHeader,
    WrongEncoding,
    WrongCpuType(u32),
    WrongCpuSubtype(u32),
    WrongFileType(u32),
    MalformedLoadCommands,
    MalformedSegment,
    MalformedSymbolTable,
    MissingSymbolTable,
    SymbolInventory(MachOSymbolInventoryValidationError),
    RelocationInventory(MachORelocationInventoryValidationError),
    FileRangeOverflow,
}

impl fmt::Display for ObjectEnvelopeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid Mach-O link-object envelope: {self:?}")
    }
}

impl std::error::Error for ObjectEnvelopeValidationError {}

#[cfg(test)]
mod tests;
