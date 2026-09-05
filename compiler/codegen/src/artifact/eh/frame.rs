//! Closed `__eh_frame` CIE/FDE decoder for LLVM 22.1 Darwin/AArch64.
//!
//! `__eh_frame` is not by itself evidence of a Scoop exception edge. LLVM
//! also emits ordinary DWARF CFI when a function's frame cannot be represented
//! by Darwin compact unwind. We qualify that unwind-only `zR` form separately
//! from Scoop's personality/LSDA-bearing `zPLR` form so the latter still has
//! to match the complete LIR invoke manifest exactly.

use std::collections::{BTreeMap, BTreeSet};

use crate::CodegenError;

use super::cursor::Cursor;
use super::{EhRelocation, EhSection, Fde};

const CIE_VERSION: u8 = 1;
const SCOOP_CIE_AUGMENTATION: &[u8] = b"zPLR";
const UNWIND_ONLY_CIE_AUGMENTATION: &[u8] = b"zR";
const CIE_CODE_ALIGNMENT: u64 = 1;
const CIE_DATA_ALIGNMENT: i64 = -8;
const AARCH64_RETURN_ADDRESS_REGISTER: u8 = 30;
const PERSONALITY_ENCODING: u8 = 0x9b;
const PCREL_ABSPTR_ENCODING: u8 = 0x10;
const ARM64_RELOC_UNSIGNED: u8 = 0;
const ARM64_RELOC_SUBTRACTOR: u8 = 1;
const ARM64_RELOC_POINTER_TO_GOT: u8 = 7;
const SCOOP_PERSONALITY_SYMBOL: &str = "_scoop_eh_personality";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CieKind {
    Scoop(ScoopCie),
    UnwindOnly { fde_encoding: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ScoopCie {
    lsda_encoding: u8,
    fde_encoding: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cie {
    offset: usize,
    kind: CieKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UnwindOnlyFde {
    pub(super) function_symbol: String,
    pub(super) function_start: u64,
    pub(super) function_size: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ParsedEhFrame {
    pub(super) scoop_fdes: Vec<Fde>,
    pub(super) unwind_only_fdes: Vec<UnwindOnlyFde>,
}

pub(super) fn parse_eh_frame(section: &EhSection) -> Result<ParsedEhFrame, CodegenError> {
    let relocations = relocation_map(section)?;
    let mut used_relocations = BTreeSet::new();
    let mut cies = BTreeMap::new();
    let mut used_cies = BTreeSet::new();
    let mut parsed = ParsedEhFrame::default();
    let mut cursor = Cursor::new(&section.bytes, "__eh_frame");
    while cursor.position() < section.bytes.len() {
        let record_offset = cursor.position();
        if section.bytes[record_offset..].iter().all(|byte| *byte == 0) {
            break;
        }
        let length = cursor.u32("record length")?;
        if length == 0xffff_ffff {
            return Err(CodegenError(
                "__eh_frame uses unsupported DWARF64 record length".to_string(),
            ));
        }
        if length < 4 {
            return Err(CodegenError(format!(
                "__eh_frame record at offset {record_offset} has invalid length {length}"
            )));
        }
        let body_start = cursor.position();
        let record_end = body_start
            .checked_add(usize::try_from(length).expect("u32 fits usize"))
            .ok_or_else(|| CodegenError("__eh_frame record range overflows".to_string()))?;
        let body = section.bytes.get(body_start..record_end).ok_or_else(|| {
            CodegenError(format!(
                "__eh_frame record at offset {record_offset} is truncated"
            ))
        })?;
        let mut record = Cursor::new(body, "__eh_frame record");
        let cie_pointer = record.u32("CIE id/pointer")?;
        if cie_pointer == 0 {
            let cie = parse_cie(
                record_offset,
                body_start,
                &mut record,
                &relocations,
                &mut used_relocations,
            )?;
            if cies.insert(record_offset, cie).is_some() {
                return Err(CodegenError(format!(
                    "__eh_frame repeats CIE offset {record_offset}"
                )));
            }
        } else {
            let pointer_field = body_start;
            let cie_offset = pointer_field
                .checked_sub(usize::try_from(cie_pointer).expect("u32 fits usize"))
                .ok_or_else(|| {
                    CodegenError(format!(
                        "FDE at offset {record_offset} has out-of-range CIE pointer {cie_pointer}"
                    ))
                })?;
            let cie = *cies.get(&cie_offset).ok_or_else(|| {
                CodegenError(format!(
                    "FDE at offset {record_offset} references unknown CIE offset {cie_offset}"
                ))
            })?;
            used_cies.insert(cie.offset);
            match cie.kind {
                CieKind::Scoop(scoop_cie) => parsed.scoop_fdes.push(parse_scoop_fde(
                    record_offset,
                    body_start,
                    &mut record,
                    scoop_cie,
                    section,
                    &relocations,
                    &mut used_relocations,
                )?),
                CieKind::UnwindOnly { fde_encoding } => {
                    parsed.unwind_only_fdes.push(parse_unwind_only_fde(
                        record_offset,
                        body_start,
                        &mut record,
                        fde_encoding,
                        section,
                        &relocations,
                        &mut used_relocations,
                    )?);
                }
            }
        }
        cursor.offset = record_end;
    }
    if parsed.scoop_fdes.is_empty() && parsed.unwind_only_fdes.is_empty() {
        return Err(CodegenError("__eh_frame contains no FDEs".to_string()));
    }
    if used_cies.len() != cies.len() {
        return Err(CodegenError(
            "__eh_frame contains an unreferenced CIE".to_string(),
        ));
    }
    let observed_offsets = relocations.keys().copied().collect::<BTreeSet<_>>();
    if used_relocations != observed_offsets {
        return Err(CodegenError(format!(
            "__eh_frame carries relocations outside qualified CIE/FDE fields: expected {used_relocations:?}, observed {observed_offsets:?}"
        )));
    }
    Ok(parsed)
}

fn parse_cie(
    record_offset: usize,
    body_start: usize,
    cursor: &mut Cursor<'_>,
    relocations: &BTreeMap<u64, Vec<&EhRelocation>>,
    used_relocations: &mut BTreeSet<u64>,
) -> Result<Cie, CodegenError> {
    if cursor.u8("CIE version")? != CIE_VERSION {
        return Err(CodegenError(format!(
            "CIE at offset {record_offset} does not use version {CIE_VERSION}"
        )));
    }
    let augmentation = cursor.nul_terminated("CIE augmentation")?;
    if cursor.uleb("CIE code alignment")? != CIE_CODE_ALIGNMENT
        || cursor.sleb("CIE data alignment")? != CIE_DATA_ALIGNMENT
        || cursor.u8("CIE return-address register")? != AARCH64_RETURN_ADDRESS_REGISTER
    {
        return Err(CodegenError(format!(
            "CIE at offset {record_offset} violates the Darwin/AArch64 alignment/register profile"
        )));
    }
    let augmentation_length = usize::try_from(cursor.uleb("CIE augmentation length")?)
        .map_err(|_| CodegenError("CIE augmentation length exceeds usize::MAX".to_string()))?;
    let augmentation_end = cursor
        .position()
        .checked_add(augmentation_length)
        .ok_or_else(|| CodegenError("CIE augmentation range overflows".to_string()))?;
    let kind = match augmentation {
        SCOOP_CIE_AUGMENTATION => {
            if cursor.u8("CIE personality encoding")? != PERSONALITY_ENCODING {
                return Err(CodegenError(format!(
                    "CIE at offset {record_offset} uses an unsupported personality encoding"
                )));
            }
            let personality_field = body_start + cursor.position();
            let raw = i32::from_le_bytes(
                cursor
                    .take(4, "CIE personality pointer")?
                    .try_into()
                    .expect("four-byte slice"),
            );
            validate_personality_relocation(personality_field, raw, relocations, used_relocations)?;
            let lsda_encoding = cursor.u8("CIE LSDA encoding")?;
            let fde_encoding = cursor.u8("CIE FDE encoding")?;
            if lsda_encoding != PCREL_ABSPTR_ENCODING || fde_encoding != PCREL_ABSPTR_ENCODING {
                return Err(CodegenError(format!(
                    "CIE at offset {record_offset} does not use pcrel absptr LSDA/FDE encodings"
                )));
            }
            CieKind::Scoop(ScoopCie {
                lsda_encoding,
                fde_encoding,
            })
        }
        UNWIND_ONLY_CIE_AUGMENTATION => {
            let fde_encoding = cursor.u8("CIE FDE encoding")?;
            if fde_encoding != PCREL_ABSPTR_ENCODING {
                return Err(CodegenError(format!(
                    "unwind-only CIE at offset {record_offset} does not use pcrel absptr FDE encoding"
                )));
            }
            CieKind::UnwindOnly { fde_encoding }
        }
        _ => {
            return Err(CodegenError(format!(
                "CIE at offset {record_offset} uses unsupported augmentation `{}`",
                String::from_utf8_lossy(augmentation)
            )));
        }
    };
    if cursor.position() != augmentation_end {
        return Err(CodegenError(format!(
            "CIE at offset {record_offset} augmentation length does not match its `{}` payload",
            String::from_utf8_lossy(augmentation)
        )));
    }
    Ok(Cie {
        offset: record_offset,
        kind,
    })
}

fn parse_scoop_fde(
    record_offset: usize,
    body_start: usize,
    cursor: &mut Cursor<'_>,
    cie: ScoopCie,
    section: &EhSection,
    relocations: &BTreeMap<u64, Vec<&EhRelocation>>,
    used_relocations: &mut BTreeSet<u64>,
) -> Result<Fde, CodegenError> {
    if cie.fde_encoding != PCREL_ABSPTR_ENCODING || cie.lsda_encoding != PCREL_ABSPTR_ENCODING {
        return Err(CodegenError(format!(
            "FDE at offset {record_offset} references an unsupported CIE"
        )));
    }
    let initial_field = body_start + cursor.position();
    let initial_raw = i64::from_le_bytes(
        cursor
            .take(8, "FDE initial location")?
            .try_into()
            .expect("eight-byte slice"),
    );
    let function = validate_pcrel_pair(
        initial_field,
        initial_raw,
        section.address,
        relocations,
        used_relocations,
        "FDE initial location",
        "__text",
    )?;
    let function_size = cursor.u64("FDE address range")?;
    let augmentation_length = usize::try_from(cursor.uleb("FDE augmentation length")?)
        .map_err(|_| CodegenError("FDE augmentation length exceeds usize::MAX".to_string()))?;
    if augmentation_length != 8 {
        return Err(CodegenError(format!(
            "FDE at offset {record_offset} has {augmentation_length}-byte augmentation; expected one 8-byte LSDA pointer"
        )));
    }
    let lsda_field = body_start + cursor.position();
    let lsda_raw = i64::from_le_bytes(
        cursor
            .take(8, "FDE LSDA pointer")?
            .try_into()
            .expect("eight-byte slice"),
    );
    let lsda = validate_pcrel_pair(
        lsda_field,
        lsda_raw,
        section.address,
        relocations,
        used_relocations,
        "FDE LSDA pointer",
        "__gcc_except_tab",
    )?;
    Ok(Fde {
        function_symbol: function.symbol.clone(),
        function_start: function.symbol_address,
        function_size,
        lsda_address: lsda.symbol_address,
    })
}

fn parse_unwind_only_fde(
    record_offset: usize,
    body_start: usize,
    cursor: &mut Cursor<'_>,
    fde_encoding: u8,
    section: &EhSection,
    relocations: &BTreeMap<u64, Vec<&EhRelocation>>,
    used_relocations: &mut BTreeSet<u64>,
) -> Result<UnwindOnlyFde, CodegenError> {
    if fde_encoding != PCREL_ABSPTR_ENCODING {
        return Err(CodegenError(format!(
            "unwind-only FDE at offset {record_offset} references an unsupported CIE"
        )));
    }
    let initial_field = body_start + cursor.position();
    let initial_raw = i64::from_le_bytes(
        cursor
            .take(8, "FDE initial location")?
            .try_into()
            .expect("eight-byte slice"),
    );
    let function = validate_pcrel_pair(
        initial_field,
        initial_raw,
        section.address,
        relocations,
        used_relocations,
        "unwind-only FDE initial location",
        "__text",
    )?;
    let function_size = cursor.u64("FDE address range")?;
    let augmentation_length = cursor.uleb("FDE augmentation length")?;
    if augmentation_length != 0 {
        return Err(CodegenError(format!(
            "unwind-only FDE at offset {record_offset} has a non-empty augmentation payload"
        )));
    }
    Ok(UnwindOnlyFde {
        function_symbol: function.symbol.clone(),
        function_start: function.symbol_address,
        function_size,
    })
}

fn relocation_map(section: &EhSection) -> Result<BTreeMap<u64, Vec<&EhRelocation>>, CodegenError> {
    let mut map: BTreeMap<u64, Vec<&EhRelocation>> = BTreeMap::new();
    for relocation in &section.relocations {
        let offset = usize::try_from(relocation.offset).map_err(|_| {
            CodegenError("__eh_frame relocation offset exceeds usize::MAX".to_string())
        })?;
        if offset >= section.bytes.len() {
            return Err(CodegenError(format!(
                "__eh_frame relocation at offset {} lies outside the section",
                relocation.offset
            )));
        }
        map.entry(relocation.offset).or_default().push(relocation);
    }
    Ok(map)
}

fn validate_personality_relocation(
    field: usize,
    raw: i32,
    relocations: &BTreeMap<u64, Vec<&EhRelocation>>,
    used: &mut BTreeSet<u64>,
) -> Result<(), CodegenError> {
    let offset = u64::try_from(field)
        .map_err(|_| CodegenError("CIE personality field offset exceeds u64::MAX".to_string()))?;
    let entries = relocations.get(&offset).ok_or_else(|| {
        CodegenError(format!(
            "CIE personality field at offset {field} has no relocation"
        ))
    })?;
    if entries.len() != 1
        || entries[0].r_type != ARM64_RELOC_POINTER_TO_GOT
        || !entries[0].r_pcrel
        || entries[0].r_length != 2
        || entries[0].symbol != SCOOP_PERSONALITY_SYMBOL
        || !entries[0].symbol_is_undefined
        || entries[0].symbol_section.is_some()
        || entries[0].symbol_is_local
    {
        return Err(CodegenError(format!(
            "CIE personality field at offset {field} has unsupported relocations {entries:?}"
        )));
    }
    let expected_raw = i32::try_from(field)
        .ok()
        .and_then(|field| field.checked_neg())
        .ok_or_else(|| CodegenError("CIE personality field offset exceeds i32".to_string()))?;
    if raw != expected_raw {
        return Err(CodegenError(format!(
            "CIE personality pcrel addend is {raw}, expected {expected_raw}"
        )));
    }
    used.insert(offset);
    Ok(())
}

fn validate_pcrel_pair<'a>(
    field: usize,
    raw: i64,
    section_address: u64,
    relocations: &'a BTreeMap<u64, Vec<&EhRelocation>>,
    used: &mut BTreeSet<u64>,
    what: &str,
    target_section: &str,
) -> Result<&'a EhRelocation, CodegenError> {
    let offset = u64::try_from(field)
        .map_err(|_| CodegenError(format!("{what} offset exceeds u64::MAX")))?;
    let entries = relocations
        .get(&offset)
        .ok_or_else(|| CodegenError(format!("{what} at offset {field} has no relocations")))?;
    if entries.len() != 2 {
        return Err(CodegenError(format!(
            "{what} at offset {field} has {} relocations, expected a subtractor/unsigned pair",
            entries.len()
        )));
    }
    let [subtractor, target] = entries.as_slice() else {
        unreachable!("entry count was checked above")
    };
    if subtractor.r_type != ARM64_RELOC_SUBTRACTOR
        || subtractor.r_pcrel
        || subtractor.r_length != 3
        || target.r_type != ARM64_RELOC_UNSIGNED
        || target.r_pcrel
        || target.r_length != 3
        || subtractor.symbol_address != section_address
        || subtractor.symbol_section.as_deref() != Some("__eh_frame")
        || subtractor.symbol_is_undefined
        || !subtractor.symbol_is_local
        || target.symbol_section.as_deref() != Some(target_section)
        || target.symbol_is_undefined
        || (target_section == "__text" && target.symbol_is_local)
        || (target_section == "__text" && !target.symbol_is_text)
        || (target_section == "__gcc_except_tab" && !target.symbol_is_local)
    {
        return Err(CodegenError(format!(
            "{what} has an unsupported Mach-O relocation pair {entries:?}"
        )));
    }
    let expected_raw = i64::try_from(field)
        .ok()
        .and_then(|field| field.checked_neg())
        .ok_or_else(|| CodegenError(format!("{what} offset exceeds i64")))?;
    if raw != expected_raw {
        return Err(CodegenError(format!(
            "{what} pcrel addend is {raw}, expected {expected_raw}"
        )));
    }
    used.insert(offset);
    Ok(target)
}
