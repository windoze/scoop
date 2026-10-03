//! Bounded structural parser for one complete LLVM stackmap v3 blob.

use std::fmt;

use super::{ProvisionalLlvmStackmapLiveOutV3, ProvisionalLlvmStackmapLocationV3};

const LLVM_STACKMAP_VERSION: u8 = 3;
const FUNCTION_RECORD_BYTES: usize = 24;
const CONSTANT_BYTES: usize = 8;
const MIN_CALLSITE_RECORD_BYTES: usize = 24;
const LOCATION_BYTES: usize = 12;
const LIVE_OUT_BYTES: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedLlvmStackmapSectionV3 {
    constants: Vec<u64>,
    functions: Vec<ParsedLlvmStackmapFunctionV3>,
}

impl ParsedLlvmStackmapSectionV3 {
    pub fn constants(&self) -> &[u64] {
        &self.constants
    }

    pub fn functions(&self) -> &[ParsedLlvmStackmapFunctionV3] {
        &self.functions
    }

    pub fn record_count(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.records.len())
            .sum()
    }

    pub(super) fn into_parts(self) -> (Vec<u64>, Vec<ParsedLlvmStackmapFunctionV3>) {
        (self.constants, self.functions)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedLlvmStackmapFunctionV3 {
    function_address_offset: u64,
    encoded_function_address: u64,
    stack_size: u64,
    records: Vec<ParsedLlvmStackmapRecordV3>,
}

impl ParsedLlvmStackmapFunctionV3 {
    pub const fn function_address_offset(&self) -> u64 {
        self.function_address_offset
    }

    pub const fn encoded_function_address(&self) -> u64 {
        self.encoded_function_address
    }

    pub const fn stack_size(&self) -> u64 {
        self.stack_size
    }

    pub fn records(&self) -> &[ParsedLlvmStackmapRecordV3] {
        &self.records
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedLlvmStackmapRecordV3 {
    record_offset: u64,
    safepoint_id: u64,
    instruction_offset: u32,
    flags: u16,
    locations: Vec<ProvisionalLlvmStackmapLocationV3>,
    live_outs: Vec<ProvisionalLlvmStackmapLiveOutV3>,
}

impl ParsedLlvmStackmapRecordV3 {
    pub const fn record_offset(&self) -> u64 {
        self.record_offset
    }

    pub const fn safepoint_id(&self) -> u64 {
        self.safepoint_id
    }

    pub const fn instruction_offset(&self) -> u32 {
        self.instruction_offset
    }

    pub const fn flags(&self) -> u16 {
        self.flags
    }

    pub fn locations(&self) -> &[ProvisionalLlvmStackmapLocationV3] {
        &self.locations
    }

    pub fn live_outs(&self) -> &[ProvisionalLlvmStackmapLiveOutV3] {
        &self.live_outs
    }
}

pub fn parse_llvm_stackmap_section_v3(
    bytes: &[u8],
) -> Result<ParsedLlvmStackmapSectionV3, LlvmStackmapSectionParseError> {
    let mut cursor = Cursor::new(bytes);
    let version = cursor.u8("version")?;
    if version != LLVM_STACKMAP_VERSION {
        return Err(LlvmStackmapSectionParseError::UnsupportedVersion(version));
    }
    cursor.zero(3, "header reserved bytes")?;
    let function_count = usize_from_u32(cursor.u32("function count")?);
    let constant_count = usize_from_u32(cursor.u32("constant count")?);
    let declared_record_count = cursor.u32("record count")?;

    cursor.ensure_array(function_count, FUNCTION_RECORD_BYTES, "function table")?;
    let mut function_headers = Vec::with_capacity(function_count);
    let mut observed_record_count = 0_u64;
    for _ in 0..function_count {
        let function_address_offset = cursor.position_u64()?;
        let encoded_function_address = cursor.u64("function address")?;
        let stack_size = cursor.u64("function stack size")?;
        let record_count = cursor.u64("function record count")?;
        observed_record_count = observed_record_count
            .checked_add(record_count)
            .ok_or(LlvmStackmapSectionParseError::RecordCountOverflow)?;
        function_headers.push(ParsedFunctionHeader {
            function_address_offset,
            encoded_function_address,
            stack_size,
            record_count,
        });
    }
    if observed_record_count != u64::from(declared_record_count) {
        return Err(LlvmStackmapSectionParseError::RecordCountMismatch {
            declared: declared_record_count,
            observed: observed_record_count,
        });
    }

    cursor.ensure_array(constant_count, CONSTANT_BYTES, "constant pool")?;
    let mut constants = Vec::with_capacity(constant_count);
    for _ in 0..constant_count {
        constants.push(cursor.u64("constant pool entry")?);
    }

    let record_count = usize_from_u32(declared_record_count);
    cursor.ensure_array(record_count, MIN_CALLSITE_RECORD_BYTES, "callsite records")?;
    let mut functions = Vec::with_capacity(function_headers.len());
    for header in function_headers {
        let function_record_count = usize::try_from(header.record_count).map_err(|_| {
            LlvmStackmapSectionParseError::CountDoesNotFitHost {
                what: "function record count",
                count: header.record_count,
            }
        })?;
        let mut records = Vec::with_capacity(function_record_count);
        for _ in 0..function_record_count {
            records.push(parse_record(&mut cursor)?);
        }
        functions.push(ParsedLlvmStackmapFunctionV3 {
            function_address_offset: header.function_address_offset,
            encoded_function_address: header.encoded_function_address,
            stack_size: header.stack_size,
            records,
        });
    }
    if cursor.position() != bytes.len() {
        return Err(LlvmStackmapSectionParseError::TrailingBytes {
            offset: cursor.position_u64()?,
            count: bytes.len() - cursor.position(),
        });
    }
    Ok(ParsedLlvmStackmapSectionV3 {
        constants,
        functions,
    })
}

fn parse_record(
    cursor: &mut Cursor<'_>,
) -> Result<ParsedLlvmStackmapRecordV3, LlvmStackmapSectionParseError> {
    let record_offset = cursor.position_u64()?;
    let safepoint_id = cursor.u64("record safepoint id")?;
    let instruction_offset = cursor.u32("record instruction offset")?;
    let flags = cursor.u16("record flags")?;
    let location_count = usize::from(cursor.u16("record location count")?);
    cursor.ensure_array(location_count, LOCATION_BYTES, "record locations")?;
    let mut locations = Vec::with_capacity(location_count);
    for _ in 0..location_count {
        locations.push(parse_location(cursor)?);
    }
    cursor.align_zero(8, "post-location padding")?;
    cursor.zero(2, "pre-live-out padding")?;
    let live_out_count = usize::from(cursor.u16("live-out count")?);
    cursor.ensure_array(live_out_count, LIVE_OUT_BYTES, "live-out table")?;
    let mut live_outs = Vec::with_capacity(live_out_count);
    for _ in 0..live_out_count {
        let dwarf_register = cursor.u16("live-out DWARF register")?;
        cursor.zero(1, "live-out reserved byte")?;
        let size = cursor.u8("live-out size")?;
        live_outs.push(ProvisionalLlvmStackmapLiveOutV3::new(dwarf_register, size));
    }
    cursor.align_zero(8, "post-live-out padding")?;
    Ok(ParsedLlvmStackmapRecordV3 {
        record_offset,
        safepoint_id,
        instruction_offset,
        flags,
        locations,
        live_outs,
    })
}

fn parse_location(
    cursor: &mut Cursor<'_>,
) -> Result<ProvisionalLlvmStackmapLocationV3, LlvmStackmapSectionParseError> {
    let kind = cursor.u8("location kind")?;
    cursor.zero(1, "location reserved byte")?;
    let size = cursor.u16("location size")?;
    let dwarf_register = cursor.u16("location DWARF register")?;
    cursor.zero(2, "location reserved word")?;
    let offset = cursor.i32("location offset")?;
    Ok(ProvisionalLlvmStackmapLocationV3::new(
        kind,
        size,
        dwarf_register,
        offset,
    ))
}

#[derive(Clone, Copy)]
struct ParsedFunctionHeader {
    function_address_offset: u64,
    encoded_function_address: u64,
    stack_size: u64,
    record_count: u64,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    const fn position(&self) -> usize {
        self.offset
    }

    fn position_u64(&self) -> Result<u64, LlvmStackmapSectionParseError> {
        u64::try_from(self.offset).map_err(|_| LlvmStackmapSectionParseError::OffsetDoesNotFitU64)
    }

    fn ensure_array(
        &self,
        count: usize,
        element_size: usize,
        what: &'static str,
    ) -> Result<(), LlvmStackmapSectionParseError> {
        let byte_count = count.checked_mul(element_size).ok_or(
            LlvmStackmapSectionParseError::ArraySizeOverflow {
                offset: self.position_u64()?,
                what,
            },
        )?;
        let end = self.offset.checked_add(byte_count).ok_or(
            LlvmStackmapSectionParseError::ArraySizeOverflow {
                offset: self.position_u64()?,
                what,
            },
        )?;
        if end > self.bytes.len() {
            return Err(LlvmStackmapSectionParseError::Truncated {
                offset: self.position_u64()?,
                what,
            });
        }
        Ok(())
    }

    fn take(
        &mut self,
        count: usize,
        what: &'static str,
    ) -> Result<&'a [u8], LlvmStackmapSectionParseError> {
        let start = self.position_u64()?;
        let end = self.offset.checked_add(count).ok_or(
            LlvmStackmapSectionParseError::ArraySizeOverflow {
                offset: start,
                what,
            },
        )?;
        let value =
            self.bytes
                .get(self.offset..end)
                .ok_or(LlvmStackmapSectionParseError::Truncated {
                    offset: start,
                    what,
                })?;
        self.offset = end;
        Ok(value)
    }

    fn u8(&mut self, what: &'static str) -> Result<u8, LlvmStackmapSectionParseError> {
        Ok(self.take(1, what)?[0])
    }

    fn u16(&mut self, what: &'static str) -> Result<u16, LlvmStackmapSectionParseError> {
        Ok(u16::from_le_bytes(
            self.take(2, what)?.try_into().expect("two-byte slice"),
        ))
    }

    fn u32(&mut self, what: &'static str) -> Result<u32, LlvmStackmapSectionParseError> {
        Ok(u32::from_le_bytes(
            self.take(4, what)?.try_into().expect("four-byte slice"),
        ))
    }

    fn i32(&mut self, what: &'static str) -> Result<i32, LlvmStackmapSectionParseError> {
        Ok(i32::from_le_bytes(
            self.take(4, what)?.try_into().expect("four-byte slice"),
        ))
    }

    fn u64(&mut self, what: &'static str) -> Result<u64, LlvmStackmapSectionParseError> {
        Ok(u64::from_le_bytes(
            self.take(8, what)?.try_into().expect("eight-byte slice"),
        ))
    }

    fn zero(
        &mut self,
        count: usize,
        what: &'static str,
    ) -> Result<(), LlvmStackmapSectionParseError> {
        let start = self.position_u64()?;
        if let Some(index) = self.take(count, what)?.iter().position(|byte| *byte != 0) {
            return Err(LlvmStackmapSectionParseError::NonZeroReserved {
                offset: start + index as u64,
                what,
            });
        }
        Ok(())
    }

    fn align_zero(
        &mut self,
        alignment: usize,
        what: &'static str,
    ) -> Result<(), LlvmStackmapSectionParseError> {
        let remainder = self.offset % alignment;
        if remainder != 0 {
            self.zero(alignment - remainder, what)?;
        }
        Ok(())
    }
}

const fn usize_from_u32(value: u32) -> usize {
    value as usize
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LlvmStackmapSectionParseError {
    UnsupportedVersion(u8),
    Truncated { offset: u64, what: &'static str },
    NonZeroReserved { offset: u64, what: &'static str },
    ArraySizeOverflow { offset: u64, what: &'static str },
    OffsetDoesNotFitU64,
    RecordCountOverflow,
    RecordCountMismatch { declared: u32, observed: u64 },
    CountDoesNotFitHost { what: &'static str, count: u64 },
    TrailingBytes { offset: u64, count: usize },
}

impl fmt::Display for LlvmStackmapSectionParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid LLVM stackmap v3 section: {self:?}")
    }
}

impl std::error::Error for LlvmStackmapSectionParseError {}

#[cfg(test)]
mod tests;
