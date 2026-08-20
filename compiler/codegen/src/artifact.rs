//! LLVM-v3 artifact verification shared by the supported object formats.
//!
//! LLVM IR verification proves the typed statepoint plan before instruction
//! selection. This module checks the machine pipeline's final stack-only
//! contract in the object that will actually be linked.

use crate::CodegenError;
use crate::statepoint::ExpectedSafepoints;
use crate::target::CodeArchitecture;
use std::collections::{BTreeMap, BTreeSet};

mod aarch64;
mod architecture;
mod eh;
mod elf;
mod macho;
mod memory_calls;
mod x86_64;

#[cfg(test)]
pub(crate) use eh::EhActionKind;
pub(crate) use eh::{ExpectedEh, expectations as eh_expectations};
pub(crate) use elf::verify_elf_artifact;
pub(crate) use macho::verify_macho_artifact;

const LOCATION_REGISTER: u8 = 1;
const LOCATION_INDIRECT: u8 = 3;
const LOCATION_CONSTANT: u8 = 4;
const LOCATION_CONSTANT_INDEX: u8 = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
struct FunctionRelocation {
    symbol: String,
    address: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ObservedSafepoint {
    function_symbol: String,
    call_pc: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ObservedSafepoints {
    sites: BTreeMap<u64, ObservedSafepoint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TextSection {
    address: u64,
    bytes: Vec<u8>,
    non_unwinding_calls: BTreeSet<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Location {
    kind: u8,
    size: u16,
    register: u16,
    offset: i32,
    constant: u64,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn position(&self) -> usize {
        self.offset
    }

    fn take(&mut self, count: usize, what: &str) -> Result<&'a [u8], CodegenError> {
        let end = self.offset.checked_add(count).ok_or_else(|| {
            CodegenError(format!(
                "LLVM stackmap {what} size overflows at offset {}",
                self.offset
            ))
        })?;
        let bytes = self.bytes.get(self.offset..end).ok_or_else(|| {
            CodegenError(format!(
                "LLVM stackmap is truncated while reading {what} at offset {}",
                self.offset
            ))
        })?;
        self.offset = end;
        Ok(bytes)
    }

    fn u8(&mut self, what: &str) -> Result<u8, CodegenError> {
        Ok(self.take(1, what)?[0])
    }

    fn u16(&mut self, what: &str) -> Result<u16, CodegenError> {
        Ok(u16::from_le_bytes(
            self.take(2, what)?.try_into().expect("two-byte slice"),
        ))
    }

    fn u32(&mut self, what: &str) -> Result<u32, CodegenError> {
        Ok(u32::from_le_bytes(
            self.take(4, what)?.try_into().expect("four-byte slice"),
        ))
    }

    fn i32(&mut self, what: &str) -> Result<i32, CodegenError> {
        Ok(i32::from_le_bytes(
            self.take(4, what)?.try_into().expect("four-byte slice"),
        ))
    }

    fn u64(&mut self, what: &str) -> Result<u64, CodegenError> {
        Ok(u64::from_le_bytes(
            self.take(8, what)?.try_into().expect("eight-byte slice"),
        ))
    }

    fn zero(&mut self, count: usize, what: &str) -> Result<(), CodegenError> {
        let start = self.offset;
        if let Some(index) = self.take(count, what)?.iter().position(|byte| *byte != 0) {
            return Err(CodegenError(format!(
                "LLVM stackmap {what} is non-zero at offset {}",
                start + index
            )));
        }
        Ok(())
    }

    fn align(&mut self, alignment: usize, what: &str) -> Result<(), CodegenError> {
        let remainder = self.offset % alignment;
        if remainder != 0 {
            self.zero(alignment - remainder, what)?;
        }
        Ok(())
    }
}

fn location(
    cursor: &mut Cursor<'_>,
    constants: &[u64],
    safepoint: u64,
) -> Result<Location, CodegenError> {
    let kind = cursor.u8("location kind")?;
    if !(LOCATION_REGISTER..=LOCATION_CONSTANT_INDEX).contains(&kind) {
        return Err(CodegenError(format!(
            "SafepointId {safepoint} has unknown stackmap location kind {kind}"
        )));
    }
    cursor.zero(1, "location reserved byte")?;
    let size = cursor.u16("location size")?;
    let register = cursor.u16("location DWARF register")?;
    cursor.zero(2, "location reserved word")?;
    let offset = cursor.i32("location offset")?;
    let constant = match kind {
        LOCATION_CONSTANT => offset as i64 as u64,
        LOCATION_CONSTANT_INDEX => {
            let index = usize::try_from(offset).map_err(|_| {
                CodegenError(format!(
                    "SafepointId {safepoint} has negative constant-pool index {offset}"
                ))
            })?;
            *constants.get(index).ok_or_else(|| {
                CodegenError(format!(
                    "SafepointId {safepoint} constant-pool index {index} is out of range"
                ))
            })?
        }
        _ => 0,
    };
    Ok(Location {
        kind,
        size,
        register,
        offset,
        constant,
    })
}

fn validate_header_location(
    location: Location,
    safepoint: u64,
    name: &str,
) -> Result<u64, CodegenError> {
    if !matches!(location.kind, LOCATION_CONSTANT | LOCATION_CONSTANT_INDEX) || location.size != 8 {
        return Err(CodegenError(format!(
            "SafepointId {safepoint} {name} header is not an 8-byte constant"
        )));
    }
    Ok(location.constant)
}

fn validate_root(
    root: Location,
    derived: Location,
    stack_size: u64,
    safepoint: u64,
    index: usize,
    architecture: CodeArchitecture,
) -> Result<(), CodegenError> {
    if root != derived {
        return Err(CodegenError(format!(
            "SafepointId {safepoint} root {index} has distinct base/derived machine locations"
        )));
    }
    if root.kind != LOCATION_INDIRECT
        || root.size != 8
        || architecture
            .root_offset(root.register, root.offset, stack_size)
            .is_none()
    {
        return Err(CodegenError(format!(
            "SafepointId {safepoint} root {index} violates {architecture:?} stack-indirect spill policy: {root:?} in {stack_size}-byte frame"
        )));
    }
    Ok(())
}

fn parse_stackmaps(
    bytes: &[u8],
    function_relocations: &BTreeMap<u64, FunctionRelocation>,
    text: &BTreeMap<String, TextSection>,
    expected: &ExpectedSafepoints,
    expected_version: u8,
    architecture: CodeArchitecture,
) -> Result<ObservedSafepoints, CodegenError> {
    let mut cursor = Cursor::new(bytes);
    let version = cursor.u8("version")?;
    if version != expected_version {
        return Err(CodegenError(format!(
            "LLVM emitted stackmap version {version}, expected {expected_version}"
        )));
    }
    cursor.zero(3, "header reserved bytes")?;
    let function_count = usize::try_from(cursor.u32("function count")?)
        .expect("u32 always fits usize on supported 64-bit targets");
    let constant_count = usize::try_from(cursor.u32("constant count")?)
        .expect("u32 always fits usize on supported 64-bit targets");
    let record_count = usize::try_from(cursor.u32("record count")?)
        .expect("u32 always fits usize on supported 64-bit targets");
    if function_count == 0 || record_count == 0 {
        return Err(CodegenError(
            "LLVM emitted an empty stackmap for managed LIR".to_string(),
        ));
    }

    let mut function_records = Vec::with_capacity(function_count);
    let mut expected_relocations = BTreeSet::new();
    let mut record_sum = 0u64;
    for index in 0..function_count {
        let relocation_offset = u64::try_from(cursor.position())
            .map_err(|_| CodegenError("stackmap section offset exceeds u64::MAX".to_string()))?;
        expected_relocations.insert(relocation_offset);
        let function = function_relocations
            .get(&relocation_offset)
            .ok_or_else(|| {
                CodegenError(format!(
                    "LLVM stackmap function record {index} has no function-address relocation"
                ))
            })?
            .clone();
        let address = cursor.u64("function address")?;
        let stack_size = cursor.u64("function stack size")?;
        let records = cursor.u64("function record count")?;
        if address != 0 {
            return Err(CodegenError(format!(
                "LLVM stackmap function record {index} has a pre-relocated address {address:#x}"
            )));
        }
        if !architecture.valid_stack_size(stack_size) {
            return Err(CodegenError(format!(
                "LLVM stackmap function record {index} has invalid stack size {stack_size}"
            )));
        }
        record_sum = record_sum.checked_add(records).ok_or_else(|| {
            CodegenError("stackmap function record count overflows u64".to_string())
        })?;
        function_records.push((function, stack_size, records));
    }
    let observed_relocations = function_relocations
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    if observed_relocations != expected_relocations {
        return Err(CodegenError(format!(
            "LLVM stackmap function-address relocations disagree with its function table: expected {expected_relocations:?}, observed {observed_relocations:?}"
        )));
    }
    if record_sum != record_count as u64 {
        return Err(CodegenError(format!(
            "stackmap function records claim {record_sum} callsites, header says {record_count}"
        )));
    }

    let mut constants = Vec::with_capacity(constant_count);
    for _ in 0..constant_count {
        constants.push(cursor.u64("large constant")?);
    }

    let mut observed = BTreeMap::new();
    for (function, stack_size, function_record_count) in function_records {
        let text = text.get(&function.symbol).ok_or_else(|| {
            CodegenError(format!(
                "stackmap function `{}` has no code extent",
                function.symbol
            ))
        })?;
        let mut frame_chain_validated = false;
        for _ in 0..function_record_count {
            let safepoint = cursor.u64("record SafepointId")?;
            let instruction_offset = cursor.u32("record instruction offset")?;
            let call_pc = architecture.validate_safepoint(
                text,
                &function,
                instruction_offset,
                safepoint,
                !frame_chain_validated,
            )?;
            frame_chain_validated = true;
            let flags = cursor.u16("record flags")?;
            let location_count = usize::from(cursor.u16("record location count")?);
            if safepoint == 0 || flags != 0 || location_count < 3 {
                return Err(CodegenError(format!(
                    "invalid stackmap record header for SafepointId {safepoint}"
                )));
            }
            if observed
                .insert(
                    safepoint,
                    ObservedSafepoint {
                        function_symbol: function.symbol.clone(),
                        call_pc,
                    },
                )
                .is_some()
            {
                return Err(CodegenError(format!(
                    "LLVM stackmap repeats SafepointId {safepoint}"
                )));
            }
            let calling_convention = location(&mut cursor, &constants, safepoint)?;
            let statepoint_flags = location(&mut cursor, &constants, safepoint)?;
            let deopt_count = location(&mut cursor, &constants, safepoint)?;
            let _calling_convention =
                validate_header_location(calling_convention, safepoint, "calling-convention")?;
            if validate_header_location(statepoint_flags, safepoint, "flags")? != 0 {
                return Err(CodegenError(format!(
                    "SafepointId {safepoint} has non-zero machine statepoint flags"
                )));
            }
            if validate_header_location(deopt_count, safepoint, "deopt-count")? != 0 {
                return Err(CodegenError(format!(
                    "SafepointId {safepoint} unexpectedly has deopt locations"
                )));
            }
            let root_location_count = location_count - 3;
            if root_location_count % 2 != 0 {
                return Err(CodegenError(format!(
                    "SafepointId {safepoint} has an unpaired GC location"
                )));
            }
            let root_count = root_location_count / 2;
            let expected_root_count = expected.root_count(safepoint).ok_or_else(|| {
                CodegenError(format!(
                    "LLVM stackmap contains unexpected SafepointId {safepoint}"
                ))
            })?;
            if root_count != expected_root_count {
                return Err(CodegenError(format!(
                    "SafepointId {safepoint} machine root count disagrees with complete LIR: expected {expected_root_count}, observed {root_count}"
                )));
            }
            for index in 0..root_count {
                let root = location(&mut cursor, &constants, safepoint)?;
                let derived = location(&mut cursor, &constants, safepoint)?;
                validate_root(root, derived, stack_size, safepoint, index, architecture)?;
            }
            cursor.align(8, "post-location padding")?;
            cursor.zero(2, "pre-live-out padding")?;
            let live_out_count = usize::from(cursor.u16("live-out count")?);
            for _ in 0..live_out_count {
                let _register = cursor.u16("live-out DWARF register")?;
                cursor.zero(1, "live-out reserved byte")?;
                let _size = cursor.u8("live-out size")?;
            }
            cursor.align(8, "post-live-out padding")?;
        }
    }
    if cursor.position() != bytes.len() {
        return Err(CodegenError(format!(
            "LLVM stackmap has {} trailing bytes",
            bytes.len() - cursor.position()
        )));
    }
    if observed.len() != expected.site_count() {
        return Err(CodegenError(format!(
            "LLVM stackmap has {} SafepointIds, complete LIR requires {}",
            observed.len(),
            expected.site_count()
        )));
    }
    Ok(ObservedSafepoints { sites: observed })
}
