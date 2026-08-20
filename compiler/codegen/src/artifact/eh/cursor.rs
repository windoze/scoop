//! Checked byte cursor shared by the closed EH decoders.

use crate::CodegenError;

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    pub(super) offset: usize,
    label: &'static str,
}

impl<'a> Cursor<'a> {
    pub(super) fn new(bytes: &'a [u8], label: &'static str) -> Self {
        Self {
            bytes,
            offset: 0,
            label,
        }
    }

    pub(super) fn at(bytes: &'a [u8], offset: usize, label: &'static str) -> Self {
        Self {
            bytes,
            offset,
            label,
        }
    }

    pub(super) fn position(&self) -> usize {
        self.offset
    }

    pub(super) fn take(&mut self, count: usize, what: &str) -> Result<&'a [u8], CodegenError> {
        let end = self.offset.checked_add(count).ok_or_else(|| {
            CodegenError(format!(
                "{} {what} size overflows at offset {}",
                self.label, self.offset
            ))
        })?;
        let bytes = self.bytes.get(self.offset..end).ok_or_else(|| {
            CodegenError(format!(
                "{} is truncated while reading {what} at offset {}",
                self.label, self.offset
            ))
        })?;
        self.offset = end;
        Ok(bytes)
    }

    pub(super) fn u8(&mut self, what: &str) -> Result<u8, CodegenError> {
        Ok(self.take(1, what)?[0])
    }

    pub(super) fn u32(&mut self, what: &str) -> Result<u32, CodegenError> {
        Ok(u32::from_le_bytes(
            self.take(4, what)?.try_into().expect("four-byte slice"),
        ))
    }

    pub(super) fn u64(&mut self, what: &str) -> Result<u64, CodegenError> {
        Ok(u64::from_le_bytes(
            self.take(8, what)?.try_into().expect("eight-byte slice"),
        ))
    }

    pub(super) fn uleb(&mut self, what: &str) -> Result<u64, CodegenError> {
        let start = self.offset;
        let mut value = 0u128;
        for shift in (0..=63).step_by(7) {
            let byte = self.u8(what)?;
            value |= u128::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return u64::try_from(value).map_err(|_| {
                    CodegenError(format!(
                        "{} {what} overflows u64 at offset {start}",
                        self.label
                    ))
                });
            }
        }
        Err(CodegenError(format!(
            "{} {what} overflows u64 at offset {start}",
            self.label
        )))
    }

    pub(super) fn sleb(&mut self, what: &str) -> Result<i64, CodegenError> {
        let start = self.offset;
        let mut value = 0i128;
        let mut shift = 0u32;
        loop {
            if shift >= 70 {
                return Err(CodegenError(format!(
                    "{} {what} overflows i64 at offset {start}",
                    self.label
                )));
            }
            let byte = self.u8(what)?;
            value |= i128::from(byte & 0x7f) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                if byte & 0x40 != 0 {
                    value |= (!0i128) << shift;
                }
                return i64::try_from(value).map_err(|_| {
                    CodegenError(format!(
                        "{} {what} overflows i64 at offset {start}",
                        self.label
                    ))
                });
            }
        }
    }

    pub(super) fn nul_terminated(&mut self, what: &str) -> Result<&'a [u8], CodegenError> {
        let start = self.offset;
        let relative = self.bytes[start..]
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| {
                CodegenError(format!(
                    "{} has unterminated {what} at offset {start}",
                    self.label
                ))
            })?;
        let end = start + relative;
        self.offset = end + 1;
        Ok(&self.bytes[start..end])
    }
}
