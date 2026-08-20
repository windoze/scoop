//! Shared fail-closed Mach-O envelope validation for built-in object handlers.

use std::fmt;
use std::mem;

use object::read::macho::MachHeader as _;
use object::read::macho::{Section as _, Segment as _};
use object::{Endianness, macho};
use scoop_wire::{Digest256, sha256};

mod tables;
use tables::*;

mod sections;
pub use sections::*;

mod symbols;
pub use symbols::*;

mod relocations;
pub use relocations::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DarwinBuildToolVersionV1 {
    tool: u32,
    version: u32,
}

impl DarwinBuildToolVersionV1 {
    pub const fn new(tool: u32, version: u32) -> Self {
        Self { tool, version }
    }

    pub const fn tool(self) -> u32 {
        self.tool
    }

    pub const fn version(self) -> u32 {
        self.version
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DarwinDeploymentCommandV1 {
    BuildVersion {
        minimum_os: u32,
        sdk: u32,
        tools: Vec<DarwinBuildToolVersionV1>,
    },
    VersionMin {
        minimum_os: u32,
        sdk: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedDarwinArm64ObjectEnvelopeV1 {
    byte_length: u64,
    content_digest: Digest256,
    load_command_count: u32,
    section_count: u32,
    symbol_count: u32,
    relocation_count: u64,
    deployment: Option<DarwinDeploymentCommandV1>,
    sections: Vec<ObservedMachOSectionV1>,
    symbols: Vec<ObservedMachOSymbolV1>,
    relocations: Vec<ObservedMachORelocationV1>,
}

impl ValidatedDarwinArm64ObjectEnvelopeV1 {
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub const fn content_digest(&self) -> Digest256 {
        self.content_digest
    }

    pub const fn load_command_count(&self) -> u32 {
        self.load_command_count
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

    pub const fn deployment(&self) -> Option<&DarwinDeploymentCommandV1> {
        self.deployment.as_ref()
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

pub fn validate_darwin_arm64_object_envelope_v1(
    bytes: &[u8],
) -> Result<ValidatedDarwinArm64ObjectEnvelopeV1, ObjectEnvelopeValidationError> {
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
    if header.cputype(endian) != macho::CPU_TYPE_ARM64 {
        return Err(ObjectEnvelopeValidationError::WrongCpuType(
            header.cputype(endian),
        ));
    }
    if header.cpusubtype(endian) != macho::CPU_SUBTYPE_ARM64_ALL {
        return Err(ObjectEnvelopeValidationError::WrongCpuSubtype(
            header.cpusubtype(endian),
        ));
    }
    if header.filetype(endian) != macho::MH_OBJECT {
        return Err(ObjectEnvelopeValidationError::WrongFileType(
            header.filetype(endian),
        ));
    }
    let flags = header.flags(endian);
    if flags & macho::MH_SUBSECTIONS_VIA_SYMBOLS == 0 {
        return Err(ObjectEnvelopeValidationError::MissingSubsectionsViaSymbols);
    }
    if flags != macho::MH_SUBSECTIONS_VIA_SYMBOLS {
        return Err(ObjectEnvelopeValidationError::UnsupportedHeaderFlags(flags));
    }
    let command_count = header.ncmds(endian);

    let mut commands = header
        .load_commands(endian, bytes, 0)
        .map_err(|_| ObjectEnvelopeValidationError::MalformedLoadCommands)?;
    let mut command_bytes = 0_u32;
    let mut segment = None;
    let mut symtab = None;
    let mut dysymtab = None;
    let mut deployment = None;
    let mut optimization_hints = false;
    let mut observed_sections = Vec::new();
    let mut occupied_ranges = vec![CheckedFileRange::new(
        0,
        (mem::size_of::<macho::MachHeader64<Endianness>>() as u64)
            .checked_add(u64::from(header.sizeofcmds(endian)))
            .ok_or(ObjectEnvelopeValidationError::MalformedLoadCommands)?,
    )?];
    let mut relocation_count = 0_u64;
    while let Some(command) = commands
        .next()
        .map_err(|_| ObjectEnvelopeValidationError::MalformedLoadCommands)?
    {
        command_bytes = command_bytes
            .checked_add(command.cmdsize())
            .ok_or(ObjectEnvelopeValidationError::MalformedLoadCommands)?;
        match command.cmd() {
            macho::LC_SEGMENT_64 => {
                if segment.is_some() {
                    return Err(ObjectEnvelopeValidationError::DuplicateLoadCommand(
                        macho::LC_SEGMENT_64,
                    ));
                }
                let (record, section_bytes) = command
                    .segment_64()
                    .map_err(|_| ObjectEnvelopeValidationError::MalformedSegment)?
                    .ok_or(ObjectEnvelopeValidationError::MalformedSegment)?;
                let section_count = record.nsects.get(endian);
                let expected_section_bytes = usize::try_from(section_count)
                    .ok()
                    .and_then(|count| {
                        count.checked_mul(mem::size_of::<macho::Section64<Endianness>>())
                    })
                    .ok_or(ObjectEnvelopeValidationError::MalformedSegment)?;
                if section_bytes.len() != expected_section_bytes {
                    return Err(ObjectEnvelopeValidationError::MalformedSegment);
                }
                let file_end = record
                    .fileoff
                    .get(endian)
                    .checked_add(record.filesize.get(endian))
                    .ok_or(ObjectEnvelopeValidationError::SegmentOutOfBounds)?;
                if file_end > byte_length {
                    return Err(ObjectEnvelopeValidationError::SegmentOutOfBounds);
                }
                let sections = record
                    .sections(endian, section_bytes)
                    .map_err(|_| ObjectEnvelopeValidationError::MalformedSegment)?;
                for section in sections {
                    observed_sections.push(validate_section(
                        section,
                        endian,
                        record.fileoff.get(endian),
                        file_end,
                        byte_length,
                        &mut relocation_count,
                        &mut occupied_ranges,
                    )?);
                }
                segment = Some(section_count);
            }
            macho::LC_SYMTAB => {
                if symtab.is_some() {
                    return Err(ObjectEnvelopeValidationError::DuplicateLoadCommand(
                        macho::LC_SYMTAB,
                    ));
                }
                let record = command
                    .symtab()
                    .map_err(|_| ObjectEnvelopeValidationError::MalformedSymbolTable)?
                    .ok_or(ObjectEnvelopeValidationError::MalformedSymbolTable)?;
                if command.cmdsize()
                    != u32::try_from(mem::size_of::<macho::SymtabCommand<Endianness>>())
                        .expect("Mach-O symbol-table command size fits u32")
                {
                    return Err(ObjectEnvelopeValidationError::MalformedSymbolTable);
                }
                validate_symbol_table(record, endian, bytes, byte_length, &mut occupied_ranges)?;
                symtab = Some(record);
            }
            macho::LC_DYSYMTAB => {
                if dysymtab.is_some() {
                    return Err(ObjectEnvelopeValidationError::DuplicateLoadCommand(
                        macho::LC_DYSYMTAB,
                    ));
                }
                let record = command
                    .dysymtab()
                    .map_err(|_| ObjectEnvelopeValidationError::MalformedDynamicSymbolTable)?
                    .ok_or(ObjectEnvelopeValidationError::MalformedDynamicSymbolTable)?;
                if command.cmdsize()
                    != u32::try_from(mem::size_of::<macho::DysymtabCommand<Endianness>>())
                        .expect("Mach-O dynamic-symbol-table command size fits u32")
                {
                    return Err(ObjectEnvelopeValidationError::MalformedDynamicSymbolTable);
                }
                dysymtab = Some(record);
            }
            macho::LC_BUILD_VERSION => {
                if deployment.is_some() {
                    return Err(ObjectEnvelopeValidationError::DuplicateDeploymentCommand);
                }
                let record = command
                    .build_version()
                    .map_err(|_| ObjectEnvelopeValidationError::MalformedDeploymentCommand)?
                    .ok_or(ObjectEnvelopeValidationError::MalformedDeploymentCommand)?;
                if record.platform.get(endian) != macho::PLATFORM_MACOS {
                    return Err(ObjectEnvelopeValidationError::WrongDeploymentPlatform(
                        record.platform.get(endian),
                    ));
                }
                let tool_count = record.ntools.get(endian);
                let tool_count = usize::try_from(tool_count)
                    .map_err(|_| ObjectEnvelopeValidationError::MalformedDeploymentCommand)?;
                let expected_size = mem::size_of::<macho::BuildVersionCommand<Endianness>>()
                    .checked_add(
                        tool_count
                            .checked_mul(mem::size_of::<macho::BuildToolVersion<Endianness>>())
                            .ok_or(ObjectEnvelopeValidationError::MalformedDeploymentCommand)?,
                    )
                    .ok_or(ObjectEnvelopeValidationError::MalformedDeploymentCommand)?;
                if usize::try_from(command.cmdsize()).ok() != Some(expected_size) {
                    return Err(ObjectEnvelopeValidationError::MalformedDeploymentCommand);
                }
                let mut tools = Vec::with_capacity(tool_count);
                for bytes in command.raw_data()
                    [mem::size_of::<macho::BuildVersionCommand<Endianness>>()..]
                    .chunks_exact(mem::size_of::<macho::BuildToolVersion<Endianness>>())
                {
                    tools.push(DarwinBuildToolVersionV1 {
                        tool: u32::from_le_bytes(bytes[..4].try_into().map_err(|_| {
                            ObjectEnvelopeValidationError::MalformedDeploymentCommand
                        })?),
                        version: u32::from_le_bytes(bytes[4..].try_into().map_err(|_| {
                            ObjectEnvelopeValidationError::MalformedDeploymentCommand
                        })?),
                    });
                }
                deployment = Some(DarwinDeploymentCommandV1::BuildVersion {
                    minimum_os: record.minos.get(endian),
                    sdk: record.sdk.get(endian),
                    tools,
                });
            }
            macho::LC_VERSION_MIN_MACOSX => {
                if deployment.is_some() {
                    return Err(ObjectEnvelopeValidationError::DuplicateDeploymentCommand);
                }
                let record = command
                    .data::<macho::VersionMinCommand<Endianness>>()
                    .map_err(|_| ObjectEnvelopeValidationError::MalformedDeploymentCommand)?;
                if command.cmdsize()
                    != u32::try_from(mem::size_of::<macho::VersionMinCommand<Endianness>>())
                        .expect("Mach-O version command size fits u32")
                {
                    return Err(ObjectEnvelopeValidationError::MalformedDeploymentCommand);
                }
                deployment = Some(DarwinDeploymentCommandV1::VersionMin {
                    minimum_os: record.version.get(endian),
                    sdk: record.sdk.get(endian),
                });
            }
            macho::LC_LINKER_OPTIMIZATION_HINT => {
                if optimization_hints {
                    return Err(ObjectEnvelopeValidationError::DuplicateLoadCommand(
                        command.cmd(),
                    ));
                }
                optimization_hints = true;
                if command.cmdsize() as usize
                    != mem::size_of::<macho::LinkeditDataCommand<Endianness>>()
                {
                    return Err(ObjectEnvelopeValidationError::MalformedLoadCommands);
                }
                let record = command
                    .data::<macho::LinkeditDataCommand<Endianness>>()
                    .map_err(|_| ObjectEnvelopeValidationError::MalformedLoadCommands)?;
                let range = CheckedFileRange::new(
                    u64::from(record.dataoff.get(endian)),
                    u64::from(record.datasize.get(endian)),
                )?;
                if range.end > byte_length {
                    return Err(ObjectEnvelopeValidationError::LinkeditDataOutOfBounds);
                }
                if !range.is_empty() {
                    occupied_ranges.push(range);
                }
            }
            macho::LC_LINKER_OPTION => {
                return Err(ObjectEnvelopeValidationError::EmbeddedLinkerOption);
            }
            other => return Err(ObjectEnvelopeValidationError::UnsupportedLoadCommand(other)),
        }
    }
    if command_bytes != header.sizeofcmds(endian) {
        return Err(ObjectEnvelopeValidationError::LoadCommandSizeMismatch {
            expected: header.sizeofcmds(endian),
            actual: command_bytes,
        });
    }

    let section_count = segment.ok_or(ObjectEnvelopeValidationError::MissingSegment)?;
    let symtab = symtab.ok_or(ObjectEnvelopeValidationError::MissingSymbolTable)?;
    let symbol_count = symtab.nsyms.get(endian);
    let dysymtab = dysymtab.ok_or(ObjectEnvelopeValidationError::MissingDynamicSymbolTable)?;
    validate_dynamic_symbol_table(dysymtab, endian, symbol_count)?;
    validate_disjoint_ranges(&mut occupied_ranges)?;
    let symbols = validate_darwin_arm64_symbol_inventory_v1(
        symtab,
        dysymtab,
        endian,
        bytes,
        &observed_sections,
    )
    .map_err(ObjectEnvelopeValidationError::SymbolInventory)?;
    let relocations =
        validate_darwin_arm64_relocation_inventory_v1(bytes, &observed_sections, symbol_count)
            .map_err(ObjectEnvelopeValidationError::RelocationInventory)?;

    Ok(ValidatedDarwinArm64ObjectEnvelopeV1 {
        byte_length,
        content_digest: sha256(bytes),
        load_command_count: command_count,
        section_count,
        symbol_count,
        relocation_count,
        deployment,
        sections: observed_sections,
        symbols,
        relocations,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectEnvelopeValidationError {
    MalformedHeader,
    WrongEncoding,
    WrongCpuType(u32),
    WrongCpuSubtype(u32),
    WrongFileType(u32),
    MissingSubsectionsViaSymbols,
    UnsupportedHeaderFlags(u32),
    MalformedLoadCommands,
    LoadCommandSizeMismatch { expected: u32, actual: u32 },
    DuplicateLoadCommand(u32),
    UnsupportedLoadCommand(u32),
    EmbeddedLinkerOption,
    MalformedSegment,
    SegmentOutOfBounds,
    NonCanonicalSectionName,
    SectionOutOfBounds,
    InvalidSectionAlignment { power: u32 },
    ZeroFillSectionHasRelocations,
    UnsupportedSectionReservedFields,
    RelocationTableOutOfBounds,
    RelocationInventory(DarwinArm64RelocationInventoryValidationError),
    MissingSegment,
    MalformedSymbolTable,
    SymbolTableOutOfBounds,
    StringTableOutOfBounds,
    InvalidStringTable,
    MissingSymbolTable,
    MalformedDynamicSymbolTable,
    InvalidDynamicSymbolPartition,
    UnexpectedDynamicLinkTable,
    SymbolInventory(DarwinArm64SymbolInventoryValidationError),
    MissingDynamicSymbolTable,
    DuplicateDeploymentCommand,
    MalformedDeploymentCommand,
    WrongDeploymentPlatform(u32),
    LinkeditDataOutOfBounds,
    FileRangeOverflow,
    OverlappingFileRanges,
}

impl fmt::Display for ObjectEnvelopeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Darwin/AArch64 link-object envelope: {self:?}"
        )
    }
}

impl std::error::Error for ObjectEnvelopeValidationError {}

#[cfg(test)]
mod tests;
