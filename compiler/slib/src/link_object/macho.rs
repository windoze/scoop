//! Shared fail-closed Mach-O envelope validation for built-in object handlers.

use std::fmt;
use std::mem;

use object::read::macho::MachHeader as _;
use object::{Endianness, macho};

const MAX_LINK_OBJECT_BYTES: u64 = 1_073_741_824;
const MAX_LOAD_COMMANDS: u32 = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinDeploymentCommandV1 {
    BuildVersion {
        minimum_os: u32,
        sdk: u32,
        tool_count: u32,
    },
    VersionMin {
        minimum_os: u32,
        sdk: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedDarwinArm64ObjectEnvelopeV1 {
    byte_length: u64,
    load_command_count: u32,
    section_count: u32,
    symbol_count: u32,
    deployment: DarwinDeploymentCommandV1,
}

impl ValidatedDarwinArm64ObjectEnvelopeV1 {
    pub const fn byte_length(self) -> u64 {
        self.byte_length
    }

    pub const fn load_command_count(self) -> u32 {
        self.load_command_count
    }

    pub const fn section_count(self) -> u32 {
        self.section_count
    }

    pub const fn symbol_count(self) -> u32 {
        self.symbol_count
    }

    pub const fn deployment(self) -> DarwinDeploymentCommandV1 {
        self.deployment
    }
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
    let mut dysymtab = false;
    let mut deployment = None;
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
                validate_symbol_table(record, endian, byte_length)?;
                symtab = Some(record.nsyms.get(endian));
            }
            macho::LC_DYSYMTAB => {
                if dysymtab {
                    return Err(ObjectEnvelopeValidationError::DuplicateLoadCommand(
                        macho::LC_DYSYMTAB,
                    ));
                }
                command
                    .dysymtab()
                    .map_err(|_| ObjectEnvelopeValidationError::MalformedDynamicSymbolTable)?
                    .ok_or(ObjectEnvelopeValidationError::MalformedDynamicSymbolTable)?;
                dysymtab = true;
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
                let expected_size = mem::size_of::<macho::BuildVersionCommand<Endianness>>()
                    .checked_add(
                        usize::try_from(tool_count)
                            .ok()
                            .and_then(|count| {
                                count.checked_mul(
                                    mem::size_of::<macho::BuildToolVersion<Endianness>>(),
                                )
                            })
                            .ok_or(ObjectEnvelopeValidationError::MalformedDeploymentCommand)?,
                    )
                    .ok_or(ObjectEnvelopeValidationError::MalformedDeploymentCommand)?;
                if usize::try_from(command.cmdsize()).ok() != Some(expected_size) {
                    return Err(ObjectEnvelopeValidationError::MalformedDeploymentCommand);
                }
                deployment = Some(DarwinDeploymentCommandV1::BuildVersion {
                    minimum_os: record.minos.get(endian),
                    sdk: record.sdk.get(endian),
                    tool_count,
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
    let symbol_count = symtab.ok_or(ObjectEnvelopeValidationError::MissingSymbolTable)?;
    if !dysymtab {
        return Err(ObjectEnvelopeValidationError::MissingDynamicSymbolTable);
    }
    let deployment = deployment.ok_or(ObjectEnvelopeValidationError::MissingDeploymentCommand)?;

    Ok(ValidatedDarwinArm64ObjectEnvelopeV1 {
        byte_length,
        load_command_count: command_count,
        section_count,
        symbol_count,
        deployment,
    })
}

fn validate_symbol_table(
    record: &macho::SymtabCommand<Endianness>,
    endian: Endianness,
    byte_length: u64,
) -> Result<(), ObjectEnvelopeValidationError> {
    let symbol_bytes = u64::from(record.nsyms.get(endian))
        .checked_mul(mem::size_of::<macho::Nlist64<Endianness>>() as u64)
        .ok_or(ObjectEnvelopeValidationError::SymbolTableOutOfBounds)?;
    let symbol_end = u64::from(record.symoff.get(endian))
        .checked_add(symbol_bytes)
        .ok_or(ObjectEnvelopeValidationError::SymbolTableOutOfBounds)?;
    let string_end = u64::from(record.stroff.get(endian))
        .checked_add(u64::from(record.strsize.get(endian)))
        .ok_or(ObjectEnvelopeValidationError::StringTableOutOfBounds)?;
    if symbol_end > byte_length {
        return Err(ObjectEnvelopeValidationError::SymbolTableOutOfBounds);
    }
    if string_end > byte_length {
        return Err(ObjectEnvelopeValidationError::StringTableOutOfBounds);
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
    MalformedLoadCommands,
    LoadCommandSizeMismatch { expected: u32, actual: u32 },
    DuplicateLoadCommand(u32),
    UnsupportedLoadCommand(u32),
    EmbeddedLinkerOption,
    MalformedSegment,
    SegmentOutOfBounds,
    MissingSegment,
    MalformedSymbolTable,
    SymbolTableOutOfBounds,
    StringTableOutOfBounds,
    MissingSymbolTable,
    MalformedDynamicSymbolTable,
    MissingDynamicSymbolTable,
    DuplicateDeploymentCommand,
    MalformedDeploymentCommand,
    WrongDeploymentPlatform(u32),
    MissingDeploymentCommand,
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
