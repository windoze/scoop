use crate::{LinkError, error};

pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    index: usize,
}
impl<'a> Cursor<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, index: 0 }
    }
    pub(crate) fn take(&mut self, length: usize) -> Result<&'a [u8], LinkError> {
        let end = self
            .index
            .checked_add(length)
            .ok_or_else(|| error("dyld byte range overflow"))?;
        let result = self
            .bytes
            .get(self.index..end)
            .ok_or_else(|| error("truncated dyld byte range"))?;
        self.index = end;
        Ok(result)
    }
    pub(crate) fn done(&self) -> bool {
        self.index == self.bytes.len()
    }
    pub(crate) fn byte(&mut self) -> Result<u8, LinkError> {
        let byte = self
            .bytes
            .get(self.index)
            .copied()
            .ok_or_else(|| error("truncated final dyld stream"))?;
        self.index += 1;
        Ok(byte)
    }
    pub(crate) fn uleb(&mut self) -> Result<u64, LinkError> {
        let mut value = 0;
        for shift in (0..=63).step_by(7) {
            let byte = self.byte()?;
            if shift == 63 && byte > 1 {
                return Err(error("dyld ULEB128 overflow"));
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(error("dyld ULEB128 overflow"))
    }
    pub(crate) fn sleb(&mut self) -> Result<i64, LinkError> {
        let mut value = 0u64;
        for shift in (0..=63).step_by(7) {
            let byte = self.byte()?;
            if shift == 63 && !matches!(byte, 0 | 0x7f) {
                return Err(error("dyld SLEB128 overflow"));
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                if byte & 0x40 != 0 && shift < 57 {
                    value |= u64::MAX << (shift + 7);
                }
                return Ok(value as i64);
            }
        }
        Err(error("dyld SLEB128 overflow"))
    }
    pub(crate) fn name(&mut self) -> Result<String, LinkError> {
        let end = self.bytes[self.index..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|length| self.index + length)
            .ok_or_else(|| error("unterminated dyld symbol"))?;
        let name = std::str::from_utf8(&self.bytes[self.index..end])
            .map_err(error)?
            .to_owned();
        self.index = end + 1;
        Ok(name)
    }
}
