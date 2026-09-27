//! Fail-closed view of the LLVM 22.1 callable Mach-O layout.

use object::macho;

use crate::CodegenError;

mod writer;

const MACH_HEADER_64_SIZE: usize = 32;
const SEGMENT_COMMAND_64_SIZE: usize = 72;
const SECTION_64_SIZE: usize = 80;
const SYMTAB_COMMAND_SIZE: usize = 24;
const DYSYMTAB_COMMAND_SIZE: usize = 80;
const NLIST_64_SIZE: usize = 16;

#[derive(Clone, Debug)]
pub(crate) struct BoundaryDefinitionV1 {
    pub(super) name: Vec<u8>,
    pub(super) section_ordinal: u8,
    pub(super) value: u64,
    pub(super) linkage: scoop_lir::LinkageClass,
}

impl BoundaryDefinitionV1 {
    pub(crate) fn new(
        name: Vec<u8>,
        section_ordinal: u8,
        value: u64,
        linkage: scoop_lir::LinkageClass,
    ) -> Self {
        Self {
            name,
            section_ordinal,
            value,
            linkage,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MachOSection {
    pub(super) ordinal: u8,
    segment: [u8; 16],
    section: [u8; 16],
    pub(super) address: u64,
    size: u64,
    pub(super) relocation_offset: u32,
    pub(super) relocation_count: u32,
}

impl MachOSection {
    pub(crate) const fn ordinal(self) -> u8 {
        self.ordinal
    }

    pub(crate) const fn address(self) -> u64 {
        self.address
    }

    pub(crate) fn checked_end(self) -> Result<u64, CodegenError> {
        let end = self.address.checked_add(self.size).ok_or_else(|| {
            CodegenError(format!(
                "callable section {},{} range overflows",
                String::from_utf8_lossy(fixed_name(&self.segment)),
                String::from_utf8_lossy(fixed_name(&self.section))
            ))
        })?;
        if end == self.address {
            return Err(CodegenError(format!(
                "callable section {},{} has zero physical extent",
                String::from_utf8_lossy(fixed_name(&self.segment)),
                String::from_utf8_lossy(fixed_name(&self.section))
            )));
        }
        Ok(end)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MachOSymbol {
    pub(super) name: Vec<u8>,
    symbol_type: u8,
    section_ordinal: u8,
    description: u16,
    value: u64,
}

impl MachOSymbol {
    fn is_external_definition(&self) -> bool {
        self.symbol_type == (macho::N_SECT | macho::N_EXT)
            && self.description & !(macho::N_NO_DEAD_STRIP | macho::N_ALT_ENTRY | macho::N_WEAK_DEF)
                == 0
    }

    pub(crate) const fn section_ordinal(&self) -> u8 {
        self.section_ordinal
    }

    pub(crate) const fn value(&self) -> u64 {
        self.value
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SymbolTableLayout {
    pub(super) command_offset: usize,
    pub(super) symbol_offset: u32,
    pub(super) symbol_count: u32,
    pub(super) string_offset: u32,
    string_size: u32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DynamicSymbolTableLayout {
    pub(super) command_offset: usize,
    local_count: u32,
    pub(super) external_definition_count: u32,
    pub(super) undefined_index: u32,
    undefined_count: u32,
}

#[derive(Debug)]
pub(crate) struct MachOLayout {
    pub(super) sections: Vec<MachOSection>,
    pub(super) symbols: Vec<MachOSymbol>,
    pub(super) symtab: SymbolTableLayout,
    pub(super) dysymtab: DynamicSymbolTableLayout,
}

impl MachOLayout {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, CodegenError> {
        if read_u32(bytes, 0)? != macho::MH_MAGIC_64
            || read_u32(bytes, 4)? != macho::CPU_TYPE_ARM64
            || read_u32(bytes, 12)? != macho::MH_OBJECT
        {
            return Err(CodegenError(
                "callable atom boundaries require Mach-O64/AArch64 MH_OBJECT bytes".to_string(),
            ));
        }
        let command_count = read_u32(bytes, 16)?;
        let command_bytes = usize::try_from(read_u32(bytes, 20)?).map_err(|_| malformed())?;
        let commands_end = MACH_HEADER_64_SIZE
            .checked_add(command_bytes)
            .ok_or_else(malformed)?;
        if commands_end > bytes.len() {
            return Err(malformed());
        }

        let mut cursor = MACH_HEADER_64_SIZE;
        let mut sections = Vec::new();
        let mut saw_segment = false;
        let mut symtab = None;
        let mut dysymtab = None;
        for _ in 0..command_count {
            let command = read_u32(bytes, cursor)?;
            let size = usize::try_from(read_u32(bytes, cursor + 4)?).map_err(|_| malformed())?;
            let end = cursor.checked_add(size).ok_or_else(malformed)?;
            if size < 8 || end > commands_end {
                return Err(malformed());
            }
            match command {
                macho::LC_SEGMENT_64 => {
                    if saw_segment || size < SEGMENT_COMMAND_64_SIZE {
                        return Err(malformed());
                    }
                    saw_segment = true;
                    parse_sections(bytes, cursor, size, &mut sections)?;
                }
                macho::LC_SYMTAB => {
                    if symtab.is_some() || size != SYMTAB_COMMAND_SIZE {
                        return Err(malformed());
                    }
                    symtab = Some(SymbolTableLayout {
                        command_offset: cursor,
                        symbol_offset: read_u32(bytes, cursor + 8)?,
                        symbol_count: read_u32(bytes, cursor + 12)?,
                        string_offset: read_u32(bytes, cursor + 16)?,
                        string_size: read_u32(bytes, cursor + 20)?,
                    });
                }
                macho::LC_DYSYMTAB => {
                    if dysymtab.is_some() || size != DYSYMTAB_COMMAND_SIZE {
                        return Err(malformed());
                    }
                    dysymtab = Some(parse_dynamic_symbol_table(bytes, cursor)?);
                }
                _ => {}
            }
            cursor = end;
        }
        if cursor != commands_end || sections.is_empty() {
            return Err(malformed());
        }
        let symtab = symtab.ok_or_else(malformed)?;
        let dysymtab = dysymtab.ok_or_else(malformed)?;
        validate_symbol_partition(symtab, dysymtab)?;
        let symbols = parse_symbols(bytes, symtab)?;
        Ok(Self {
            sections,
            symbols,
            symtab,
            dysymtab,
        })
    }

    pub(crate) fn require_external_definition(
        &self,
        name: &[u8],
    ) -> Result<&MachOSymbol, CodegenError> {
        let matches = self
            .symbols
            .iter()
            .filter(|symbol| symbol.name == name)
            .collect::<Vec<_>>();
        let [symbol] = matches.as_slice() else {
            return Err(CodegenError(format!(
                "callable object requires exactly one symbol `{}`",
                String::from_utf8_lossy(name)
            )));
        };
        if !symbol.is_external_definition()
            || symbol.section_ordinal == 0
            || usize::from(symbol.section_ordinal) > self.sections.len()
        {
            return Err(CodegenError(format!(
                "callable object symbol `{}` is not an external section definition (type={:#x}, section={}, description={:#x}, value={})",
                String::from_utf8_lossy(name),
                symbol.symbol_type,
                symbol.section_ordinal,
                symbol.description,
                symbol.value
            )));
        }
        Ok(symbol)
    }

    pub(crate) fn require_existing_boundary_pair(
        &self,
        start_name: &[u8],
        end_name: &[u8],
    ) -> Result<(), CodegenError> {
        let start = self.require_external_definition(start_name)?;
        let end = self.require_external_definition(end_name)?;
        if start.section_ordinal != end.section_ordinal || start.value >= end.value {
            return Err(CodegenError(format!(
                "callable global atom boundary `{}`..`{}` is not one non-empty section range",
                String::from_utf8_lossy(start_name),
                String::from_utf8_lossy(end_name)
            )));
        }
        Ok(())
    }

    pub(crate) fn find_section(
        &self,
        segment: &[u8],
        section: &[u8],
    ) -> Result<Option<MachOSection>, CodegenError> {
        let matches = self
            .sections
            .iter()
            .copied()
            .filter(|candidate| {
                fixed_name(&candidate.segment) == segment
                    && fixed_name(&candidate.section) == section
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [found] => Ok(Some(*found)),
            _ => Err(CodegenError(format!(
                "callable object repeats section {},{}",
                String::from_utf8_lossy(segment),
                String::from_utf8_lossy(section)
            ))),
        }
    }

    pub(crate) fn require_section(
        &self,
        segment: &[u8],
        section: &[u8],
    ) -> Result<MachOSection, CodegenError> {
        self.find_section(segment, section)?.ok_or_else(|| {
            CodegenError(format!(
                "callable object is missing planned section {},{}",
                String::from_utf8_lossy(segment),
                String::from_utf8_lossy(section)
            ))
        })
    }

    pub(crate) fn require_section_ordinal(
        &self,
        ordinal: u8,
    ) -> Result<MachOSection, CodegenError> {
        if ordinal == 0 {
            return Err(CodegenError(
                "Mach-O boundary symbol has no section".to_string(),
            ));
        }
        self.sections
            .get(usize::from(ordinal) - 1)
            .copied()
            .ok_or_else(malformed)
    }
}

fn parse_sections(
    bytes: &[u8],
    command_offset: usize,
    command_size: usize,
    sections: &mut Vec<MachOSection>,
) -> Result<(), CodegenError> {
    let count = usize::try_from(read_u32(bytes, command_offset + 64)?).map_err(|_| malformed())?;
    let expected = SEGMENT_COMMAND_64_SIZE
        .checked_add(count.checked_mul(SECTION_64_SIZE).ok_or_else(malformed)?)
        .ok_or_else(malformed)?;
    if command_size != expected || count > usize::from(u8::MAX) {
        return Err(malformed());
    }
    for index in 0..count {
        let offset = command_offset
            .checked_add(SEGMENT_COMMAND_64_SIZE)
            .and_then(|offset| offset.checked_add(index.checked_mul(SECTION_64_SIZE)?))
            .ok_or_else(malformed)?;
        sections.push(MachOSection {
            ordinal: u8::try_from(index + 1).map_err(|_| malformed())?,
            section: read_fixed_name(bytes, offset)?,
            segment: read_fixed_name(bytes, offset + 16)?,
            address: read_u64(bytes, offset + 32)?,
            size: read_u64(bytes, offset + 40)?,
            relocation_offset: read_u32(bytes, offset + 56)?,
            relocation_count: read_u32(bytes, offset + 60)?,
        });
    }
    Ok(())
}

fn parse_dynamic_symbol_table(
    bytes: &[u8],
    command_offset: usize,
) -> Result<DynamicSymbolTableLayout, CodegenError> {
    for offset in (32..DYSYMTAB_COMMAND_SIZE).step_by(4) {
        if read_u32(bytes, command_offset + offset)? != 0 {
            return Err(CodegenError(
                "callable object uses unsupported dynamic symbol side tables".to_string(),
            ));
        }
    }
    let local_index = read_u32(bytes, command_offset + 8)?;
    let local_count = read_u32(bytes, command_offset + 12)?;
    let external_index = read_u32(bytes, command_offset + 16)?;
    if local_index != 0 || external_index != local_count {
        return Err(CodegenError(
            "callable object has a noncanonical dynamic symbol partition".to_string(),
        ));
    }
    Ok(DynamicSymbolTableLayout {
        command_offset,
        local_count,
        external_definition_count: read_u32(bytes, command_offset + 20)?,
        undefined_index: read_u32(bytes, command_offset + 24)?,
        undefined_count: read_u32(bytes, command_offset + 28)?,
    })
}

fn validate_symbol_partition(
    symtab: SymbolTableLayout,
    dysymtab: DynamicSymbolTableLayout,
) -> Result<(), CodegenError> {
    if dysymtab
        .local_count
        .checked_add(dysymtab.external_definition_count)
        != Some(dysymtab.undefined_index)
        || dysymtab
            .undefined_index
            .checked_add(dysymtab.undefined_count)
            != Some(symtab.symbol_count)
    {
        return Err(CodegenError(
            "callable object has a noncanonical dynamic symbol partition".to_string(),
        ));
    }
    Ok(())
}

fn parse_symbols(
    bytes: &[u8],
    symtab: SymbolTableLayout,
) -> Result<Vec<MachOSymbol>, CodegenError> {
    let symbol_bytes = usize::try_from(symtab.symbol_count)
        .ok()
        .and_then(|count| count.checked_mul(NLIST_64_SIZE))
        .ok_or_else(malformed)?;
    let symbol_start = usize::try_from(symtab.symbol_offset).map_err(|_| malformed())?;
    let symbol_end = symbol_start
        .checked_add(symbol_bytes)
        .ok_or_else(malformed)?;
    let string_start = usize::try_from(symtab.string_offset).map_err(|_| malformed())?;
    let string_end = string_start
        .checked_add(usize::try_from(symtab.string_size).map_err(|_| malformed())?)
        .ok_or_else(malformed)?;
    if symbol_end != string_start || string_end != bytes.len() || symbol_start > bytes.len() {
        return Err(CodegenError(
            "callable object symbol and string tables are not the canonical contiguous tail"
                .to_string(),
        ));
    }
    let strings = bytes.get(string_start..string_end).ok_or_else(malformed)?;
    if strings.first() != Some(&0) {
        return Err(malformed());
    }
    let mut symbols = Vec::with_capacity(symtab.symbol_count as usize);
    for index in 0..symtab.symbol_count as usize {
        let offset = symbol_start
            .checked_add(index.checked_mul(NLIST_64_SIZE).ok_or_else(malformed)?)
            .ok_or_else(malformed)?;
        let name_index = usize::try_from(read_u32(bytes, offset)?).map_err(|_| malformed())?;
        let name_tail = strings.get(name_index..).ok_or_else(malformed)?;
        let name_end = name_tail
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(malformed)?;
        if name_end == 0 {
            return Err(malformed());
        }
        symbols.push(MachOSymbol {
            name: name_tail[..name_end].to_vec(),
            symbol_type: *bytes.get(offset + 4).ok_or_else(malformed)?,
            section_ordinal: *bytes.get(offset + 5).ok_or_else(malformed)?,
            description: read_u16(bytes, offset + 6)?,
            value: read_u64(bytes, offset + 8)?,
        });
    }
    Ok(symbols)
}

fn fixed_name(name: &[u8; 16]) -> &[u8] {
    &name[..name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(name.len())]
}

fn read_fixed_name(bytes: &[u8], offset: usize) -> Result<[u8; 16], CodegenError> {
    bytes
        .get(offset..offset + 16)
        .ok_or_else(malformed)?
        .try_into()
        .map_err(|_| malformed())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, CodegenError> {
    bytes
        .get(offset..offset + 2)
        .ok_or_else(malformed)?
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| malformed())
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, CodegenError> {
    bytes
        .get(offset..offset + 4)
        .ok_or_else(malformed)?
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| malformed())
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, CodegenError> {
    bytes
        .get(offset..offset + 8)
        .ok_or_else(malformed)?
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| malformed())
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), CodegenError> {
    bytes
        .get_mut(offset..offset + 4)
        .ok_or_else(malformed)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn malformed() -> CodegenError {
    CodegenError("malformed callable Mach-O object".to_string())
}
