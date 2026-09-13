//! Shared fail-closed Mach-O envelope validation for built-in object handlers.

use std::fmt;
use std::mem;

use object::read::macho::MachHeader as _;
use object::read::macho::{Section as _, Segment as _};
use object::{Endianness, macho};
use scoop_wire::{Digest256, sha256};

const MAX_LINK_OBJECT_BYTES: u64 = 1_073_741_824;
const MAX_LOAD_COMMANDS: u32 = 65_536;
const MAX_OBJECT_TABLE_ENTRIES: u64 = 16_777_216;
const MAX_OBJECT_STRING_TABLE_BYTES: u64 = 16_777_216;

mod sections;
pub use sections::*;

mod symbols;
pub use symbols::*;

mod relocations;
pub use relocations::*;

mod profiles;
pub use profiles::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DarwinBuildToolVersionV1 {
    tool: u32,
    version: u32,
}

impl DarwinBuildToolVersionV1 {
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

    pub(super) const fn file_offset(self) -> Option<u64> {
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
    let byte_length = u64::try_from(bytes.len())
        .map_err(|_| ObjectEnvelopeValidationError::ObjectTooLarge { actual: u64::MAX })?;
    if byte_length > MAX_LINK_OBJECT_BYTES {
        return Err(ObjectEnvelopeValidationError::ObjectTooLarge {
            actual: byte_length,
        });
    }

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
    if command_count > MAX_LOAD_COMMANDS {
        return Err(ObjectEnvelopeValidationError::TooManyLoadCommands {
            actual: command_count,
        });
    }

    let mut commands = header
        .load_commands(endian, bytes, 0)
        .map_err(|_| ObjectEnvelopeValidationError::MalformedLoadCommands)?;
    let mut command_bytes = 0_u32;
    let mut segment = None;
    let mut symtab = None;
    let mut dysymtab = None;
    let mut deployment = None;
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
                if u64::from(section_count) > MAX_OBJECT_TABLE_ENTRIES {
                    return Err(ObjectEnvelopeValidationError::TooManySections {
                        actual: section_count,
                    });
                }
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

fn validate_symbol_table(
    record: &macho::SymtabCommand<Endianness>,
    endian: Endianness,
    bytes: &[u8],
    byte_length: u64,
    occupied_ranges: &mut Vec<CheckedFileRange>,
) -> Result<(), ObjectEnvelopeValidationError> {
    let symbol_count = record.nsyms.get(endian);
    if u64::from(symbol_count) > MAX_OBJECT_TABLE_ENTRIES {
        return Err(ObjectEnvelopeValidationError::TooManySymbols {
            actual: symbol_count,
        });
    }
    let symbol_bytes = u64::from(symbol_count)
        .checked_mul(mem::size_of::<macho::Nlist64<Endianness>>() as u64)
        .ok_or(ObjectEnvelopeValidationError::SymbolTableOutOfBounds)?;
    let symbol_range = CheckedFileRange::new(u64::from(record.symoff.get(endian)), symbol_bytes)
        .map_err(|_| ObjectEnvelopeValidationError::SymbolTableOutOfBounds)?;
    if symbol_range.end > byte_length {
        return Err(ObjectEnvelopeValidationError::SymbolTableOutOfBounds);
    }
    let string_size = u64::from(record.strsize.get(endian));
    if string_size > MAX_OBJECT_STRING_TABLE_BYTES {
        return Err(ObjectEnvelopeValidationError::StringTableTooLarge {
            actual: string_size,
        });
    }
    let string_range = CheckedFileRange::new(u64::from(record.stroff.get(endian)), string_size)
        .map_err(|_| ObjectEnvelopeValidationError::StringTableOutOfBounds)?;
    if string_range.end > byte_length {
        return Err(ObjectEnvelopeValidationError::StringTableOutOfBounds);
    }
    if string_range.is_empty() || bytes.get(string_range.start as usize).copied().unwrap_or(1) != 0
    {
        return Err(ObjectEnvelopeValidationError::InvalidStringTable);
    }
    occupied_ranges.extend(
        [symbol_range, string_range]
            .into_iter()
            .filter(|range| !range.is_empty()),
    );
    Ok(())
}

fn validate_section(
    section: &macho::Section64<Endianness>,
    endian: Endianness,
    segment_start: u64,
    segment_end: u64,
    byte_length: u64,
    relocation_count: &mut u64,
    occupied_ranges: &mut Vec<CheckedFileRange>,
) -> Result<ObservedMachOSectionV1, ObjectEnvelopeValidationError> {
    if !has_canonical_fixed_name_padding(&section.sectname)
        || !has_canonical_fixed_name_padding(&section.segname)
    {
        return Err(ObjectEnvelopeValidationError::NonCanonicalSectionName);
    }
    if let Some((offset, size)) = section.file_range(endian) {
        let range = CheckedFileRange::new(offset, size)
            .map_err(|_| ObjectEnvelopeValidationError::SectionOutOfBounds)?;
        if range.end > byte_length || range.start < segment_start || range.end > segment_end {
            return Err(ObjectEnvelopeValidationError::SectionOutOfBounds);
        }
        let alignment_power = section.align.get(endian);
        if alignment_power >= u64::BITS
            || (!range.is_empty() && range.start % (1_u64 << alignment_power) != 0)
        {
            return Err(ObjectEnvelopeValidationError::InvalidSectionAlignment {
                power: alignment_power,
            });
        }
        if !range.is_empty() {
            occupied_ranges.push(range);
        }
    } else if section.nreloc.get(endian) != 0 {
        return Err(ObjectEnvelopeValidationError::ZeroFillSectionHasRelocations);
    }

    let count = section.nreloc.get(endian);
    *relocation_count = relocation_count
        .checked_add(u64::from(count))
        .ok_or(ObjectEnvelopeValidationError::TooManyRelocations { actual: u64::MAX })?;
    if *relocation_count > MAX_OBJECT_TABLE_ENTRIES {
        return Err(ObjectEnvelopeValidationError::TooManyRelocations {
            actual: *relocation_count,
        });
    }
    let relocation_bytes = u64::from(count)
        .checked_mul(mem::size_of::<macho::Relocation<Endianness>>() as u64)
        .ok_or(ObjectEnvelopeValidationError::RelocationTableOutOfBounds)?;
    let range = CheckedFileRange::new(u64::from(section.reloff.get(endian)), relocation_bytes)
        .map_err(|_| ObjectEnvelopeValidationError::RelocationTableOutOfBounds)?;
    if range.end > byte_length {
        return Err(ObjectEnvelopeValidationError::RelocationTableOutOfBounds);
    }
    if !range.is_empty() {
        occupied_ranges.push(range);
    }
    if section.reserved1.get(endian) != 0
        || section.reserved2.get(endian) != 0
        || section.reserved3.get(endian) != 0
    {
        return Err(ObjectEnvelopeValidationError::UnsupportedSectionReservedFields);
    }
    Ok(ObservedMachOSectionV1 {
        segment_name: section.segname,
        section_name: section.sectname,
        virtual_address: section.addr.get(endian),
        file_offset: section.file_range(endian).map(|(offset, _)| offset),
        relocation_file_offset: u64::from(section.reloff.get(endian)),
        flags: section.flags.get(endian),
        byte_size: section.size.get(endian),
        alignment_power: section.align.get(endian),
        relocation_count: count,
    })
}

fn has_canonical_fixed_name_padding(name: &[u8; 16]) -> bool {
    name.iter()
        .position(|byte| *byte == 0)
        .is_none_or(|first_zero| name[first_zero..].iter().all(|byte| *byte == 0))
}

fn validate_dynamic_symbol_table(
    record: &macho::DysymtabCommand<Endianness>,
    endian: Endianness,
    symbol_count: u32,
) -> Result<(), ObjectEnvelopeValidationError> {
    let local_end = record
        .ilocalsym
        .get(endian)
        .checked_add(record.nlocalsym.get(endian));
    let external_end = record
        .iextdefsym
        .get(endian)
        .checked_add(record.nextdefsym.get(endian));
    let undefined_end = record
        .iundefsym
        .get(endian)
        .checked_add(record.nundefsym.get(endian));
    if record.ilocalsym.get(endian) != 0
        || local_end != Some(record.iextdefsym.get(endian))
        || external_end != Some(record.iundefsym.get(endian))
        || undefined_end != Some(symbol_count)
    {
        return Err(ObjectEnvelopeValidationError::InvalidDynamicSymbolPartition);
    }
    if [
        record.tocoff.get(endian),
        record.ntoc.get(endian),
        record.modtaboff.get(endian),
        record.nmodtab.get(endian),
        record.extrefsymoff.get(endian),
        record.nextrefsyms.get(endian),
        record.indirectsymoff.get(endian),
        record.nindirectsyms.get(endian),
        record.extreloff.get(endian),
        record.nextrel.get(endian),
        record.locreloff.get(endian),
        record.nlocrel.get(endian),
    ]
    .iter()
    .any(|value| *value != 0)
    {
        return Err(ObjectEnvelopeValidationError::UnexpectedDynamicLinkTable);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CheckedFileRange {
    start: u64,
    end: u64,
}

impl CheckedFileRange {
    fn new(start: u64, size: u64) -> Result<Self, ObjectEnvelopeValidationError> {
        let end = start
            .checked_add(size)
            .ok_or(ObjectEnvelopeValidationError::FileRangeOverflow)?;
        Ok(Self { start, end })
    }

    const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

fn validate_disjoint_ranges(
    ranges: &mut [CheckedFileRange],
) -> Result<(), ObjectEnvelopeValidationError> {
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].end > pair[1].start) {
        return Err(ObjectEnvelopeValidationError::OverlappingFileRanges);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectEnvelopeValidationError {
    ObjectTooLarge { actual: u64 },
    MalformedHeader,
    WrongEncoding,
    WrongCpuType(u32),
    WrongCpuSubtype(u32),
    WrongFileType(u32),
    MissingSubsectionsViaSymbols,
    UnsupportedHeaderFlags(u32),
    TooManyLoadCommands { actual: u32 },
    TooManySections { actual: u32 },
    TooManySymbols { actual: u32 },
    TooManyRelocations { actual: u64 },
    StringTableTooLarge { actual: u64 },
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
