//! Closed LLVM 22.1 LSDA decoder used by object qualification.

use std::collections::BTreeSet;
use std::ops::Range;

use crate::CodegenError;
use crate::target::{CodeArchitecture, LsdaEncodingProfile};

use super::cursor::Cursor;
use super::{EhActionKind, ObservedLsda, ObservedProtectedRange};

pub(super) fn parse_lsda(
    bytes: &[u8],
    function_size: u64,
    encodings: LsdaEncodingProfile,
    architecture: CodeArchitecture,
) -> Result<ObservedLsda, CodegenError> {
    let alignment = architecture.instruction_alignment();
    let mut cursor = Cursor::new(bytes, "LSDA");
    let lp_start = cursor.u8("LPStart encoding")?;
    if lp_start != encodings.lp_start {
        return Err(CodegenError(format!(
            "LPStart encoding is {lp_start:#04x}, expected {:#04x}",
            encodings.lp_start
        )));
    }
    let type_table = cursor.u8("type-table encoding")?;
    let has_type_table = type_table != 0xff;
    if has_type_table && type_table != encodings.type_table {
        return Err(CodegenError(format!(
            "type-table encoding is {type_table:#04x}, expected {:#04x}",
            encodings.type_table
        )));
    }
    let type_table_base = if has_type_table {
        let offset = usize::try_from(cursor.uleb("type-table offset")?)
            .map_err(|_| CodegenError("LSDA type-table offset exceeds usize::MAX".to_string()))?;
        cursor
            .position()
            .checked_add(offset)
            .ok_or_else(|| CodegenError("LSDA type-table address overflows".to_string()))?
    } else {
        bytes.len()
    };
    if type_table_base != bytes.len() {
        return Err(CodegenError(format!(
            "LSDA type-table base is {type_table_base}, expected exact LSDA end {}",
            bytes.len()
        )));
    }
    let call_site = cursor.u8("call-site encoding")?;
    if call_site != encodings.call_site {
        return Err(CodegenError(format!(
            "call-site encoding is {call_site:#04x}, expected {:#04x}",
            encodings.call_site
        )));
    }
    let call_site_length = usize::try_from(cursor.uleb("call-site table length")?)
        .map_err(|_| CodegenError("LSDA call-site length exceeds usize::MAX".to_string()))?;
    let call_site_end = cursor
        .position()
        .checked_add(call_site_length)
        .ok_or_else(|| CodegenError("LSDA call-site table range overflows".to_string()))?;
    let type_entry_start = type_table_base
        .checked_sub(if has_type_table { 4 } else { 0 })
        .ok_or_else(|| {
            CodegenError("LSDA type table cannot contain its catch-all entry".to_string())
        })?;
    if call_site_end > type_entry_start {
        return Err(CodegenError(
            "LSDA call-site table overlaps action/type data".to_string(),
        ));
    }

    let mut call_sites = Vec::new();
    let mut previous_end = 0u64;
    while cursor.position() < call_site_end {
        let start = cursor.uleb("call-site start")?;
        let length = cursor.uleb("call-site length")?;
        let landing_pad = cursor.uleb("call-site landing pad")?;
        let action = cursor.uleb("call-site action")?;
        if cursor.position() > call_site_end {
            return Err(CodegenError(
                "LSDA call-site entry crosses the declared table end".to_string(),
            ));
        }
        let end = start
            .checked_add(length)
            .ok_or_else(|| CodegenError("LSDA call-site function range overflows".to_string()))?;
        if length == 0
            || start % alignment != 0
            || length % alignment != 0
            || start < previous_end
            || end > function_size
        {
            return Err(CodegenError(format!(
                "LSDA call-site range {start}..{end} is empty, unaligned, overlapping, or outside function size {function_size}"
            )));
        }
        if landing_pad != 0 && (landing_pad >= function_size || landing_pad % alignment != 0) {
            return Err(CodegenError(format!(
                "LSDA landing-pad offset {landing_pad} is outside/alignment-invalid for function size {function_size}"
            )));
        }
        if landing_pad == 0 && action != 0 {
            return Err(CodegenError(format!(
                "LSDA call-site without a landing pad has action {action}"
            )));
        }
        if !has_type_table && action != 0 {
            return Err(CodegenError(
                "LSDA without a type table must have zero call-site actions".to_string(),
            ));
        }
        previous_end = end;
        call_sites.push((start..end, landing_pad, action));
    }
    if cursor.position() != call_site_end {
        return Err(CodegenError(
            "LSDA call-site table length ends inside an entry".to_string(),
        ));
    }

    let action_table_start = call_site_end;
    let type_entry = bytes
        .get(type_entry_start..type_table_base)
        .ok_or_else(|| CodegenError("LSDA catch-all type entry is truncated".to_string()))?;
    if has_type_table && type_entry != [0, 0, 0, 0] {
        return Err(CodegenError(
            "LSDA catch-all type entry is not null".to_string(),
        ));
    }
    let mut actions = BTreeSet::new();
    let mut protected_ranges = Vec::new();
    let mut action_ranges = Vec::new();
    let mut action_offsets = BTreeSet::new();
    for (range, landing_pad, action) in call_sites {
        if landing_pad == 0 {
            continue;
        }
        let action_kind = if action == 0 {
            actions.insert(EhActionKind::Cleanup);
            EhActionKind::Cleanup
        } else {
            let action_minus_one = action
                .checked_sub(1)
                .expect("positive action has a predecessor");
            let action_offset = action_table_start
                .checked_add(usize::try_from(action_minus_one).map_err(|_| {
                    CodegenError("LSDA action offset exceeds usize::MAX".to_string())
                })?)
                .ok_or_else(|| CodegenError("LSDA action address overflows".to_string()))?;
            if action_offset >= type_entry_start {
                return Err(CodegenError(format!(
                    "LSDA action {action} points outside the action table"
                )));
            }
            if action_offsets.insert(action_offset) {
                let mut action_cursor =
                    Cursor::at(&bytes[..type_entry_start], action_offset, "LSDA");
                let type_filter = action_cursor.sleb("action type filter")?;
                let next = action_cursor.sleb("action next offset")?;
                if type_filter != 1 {
                    return Err(CodegenError(format!(
                        "LSDA action uses typed/filter selector {type_filter}; only catch-all selector 1 is supported"
                    )));
                }
                if next != 0 {
                    return Err(CodegenError(format!(
                        "LSDA catch-all action chains by {next}; only one terminal action is supported"
                    )));
                }
                action_ranges.push(action_offset..action_cursor.position());
            }
            actions.insert(EhActionKind::CatchAll);
            EhActionKind::CatchAll
        };
        protected_ranges.push(ObservedProtectedRange {
            range,
            action: action_kind,
            landing_pad,
        });
    }
    if action_offsets.len() > 1 {
        return Err(CodegenError(format!(
            "LSDA contains {} distinct catch actions; the closed profile requires one shared catch-all action",
            action_offsets.len()
        )));
    }
    validate_action_padding(bytes, action_table_start..type_entry_start, &action_ranges)?;
    Ok(ObservedLsda {
        actions,
        protected_ranges,
    })
}

fn validate_action_padding(
    bytes: &[u8],
    range: Range<usize>,
    records: &[Range<usize>],
) -> Result<(), CodegenError> {
    let padding_start = match records {
        [] => range.start,
        [record] if record.start == range.start && record.end <= range.end => record.end,
        _ => {
            return Err(CodegenError(
                "LSDA catch-all action is not the single leading action-table record".to_string(),
            ));
        }
    };
    if range.end - padding_start > 3 {
        return Err(CodegenError(
            "LSDA action table has more than three bytes of alignment padding".to_string(),
        ));
    }
    for index in range {
        let covered = records
            .iter()
            .any(|record| record.start <= index && index < record.end);
        if !covered && bytes[index] != 0 {
            return Err(CodegenError(format!(
                "LSDA action-table padding is non-zero at offset {index}"
            )));
        }
    }
    Ok(())
}
