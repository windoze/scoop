//! The classic dyld opcode streams selected by -no_fixup_chains.
use super::*;

mod bind;
pub(super) use bind::{Binding, bindings};

pub(super) fn rebases(bytes: &[u8], segments: &[Segment]) -> Result<BTreeSet<u64>, LinkError> {
    let mut cursor = Cursor::new(bytes);
    let mut position = Position::default();
    let mut result = BTreeSet::new();
    while !cursor.done() {
        let byte = cursor.byte()?;
        let immediate = u64::from(byte & 0xf);
        let (count, skip) = match byte & 0xf0 {
            0x00 => continue,
            0x10 if immediate == 1 => continue,
            0x20 => {
                position.segment = immediate as usize;
                position.offset = cursor.uleb()?;
                continue;
            }
            0x30 => {
                position.advance(cursor.uleb()?)?;
                continue;
            }
            0x40 => {
                position.advance(immediate * 8)?;
                continue;
            }
            0x50 => (immediate, 0),
            0x60 => (cursor.uleb()?, 0),
            0x70 => (1, cursor.uleb()?),
            0x80 => (cursor.uleb()?, cursor.uleb()?),
            _ => return Err(error(format!("invalid final rebase opcode {byte:#x}"))),
        };
        position.check_run(segments, count, skip)?;
        for _ in 0..count {
            let address = position.address(segments)?;
            if !result.insert(address) {
                return Err(error("duplicate final rebase"));
            }
            position.advance(
                8u64.checked_add(skip)
                    .ok_or_else(|| error("rebase skip overflow"))?,
            )?;
        }
    }
    Ok(result)
}

#[derive(Default)]
struct Position {
    segment: usize,
    offset: u64,
}
impl Position {
    fn address(&self, segments: &[Segment]) -> Result<u64, LinkError> {
        let segment = segments
            .get(self.segment)
            .ok_or_else(|| error("fixup segment is out of bounds"))?;
        if self
            .offset
            .checked_add(8)
            .is_none_or(|end| end > segment.file_size)
        {
            return Err(error("fixup pointer is outside its segment"));
        }
        segment
            .address
            .checked_add(self.offset)
            .ok_or_else(|| error("fixup VM address overflow"))
    }
    fn advance(&mut self, amount: u64) -> Result<(), LinkError> {
        // dyld applies an unsigned machine-word delta. ld also uses the
        // two's-complement form to move backwards between symbol groups.
        // Every resulting pointer location is bounds-checked by address().
        self.offset = self.offset.wrapping_add(amount);
        Ok(())
    }
    fn check_run(&self, segments: &[Segment], count: u64, skip: u64) -> Result<(), LinkError> {
        if count == 0 {
            return Ok(());
        }
        let last = count
            .checked_sub(1)
            .and_then(|count| count.checked_mul(skip.checked_add(8)?))
            .and_then(|offset| offset.checked_add(self.offset))
            .ok_or_else(|| error("fixup run overflows"))?;
        Position {
            segment: self.segment,
            offset: last,
        }
        .address(segments)?;
        Ok(())
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    index: usize,
}
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, index: 0 }
    }
    fn done(&self) -> bool {
        self.index == self.bytes.len()
    }
    fn byte(&mut self) -> Result<u8, LinkError> {
        let byte = self
            .bytes
            .get(self.index)
            .copied()
            .ok_or_else(|| error("truncated final fixup stream"))?;
        self.index += 1;
        Ok(byte)
    }
    fn uleb(&mut self) -> Result<u64, LinkError> {
        let mut value = 0;
        for shift in (0..=63).step_by(7) {
            let byte = self.byte()?;
            if shift == 63 && byte > 1 {
                return Err(error("fixup ULEB128 overflow"));
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(error("fixup ULEB128 overflow"))
    }
    fn sleb(&mut self) -> Result<i64, LinkError> {
        let mut value = 0u64;
        for shift in (0..=63).step_by(7) {
            let byte = self.byte()?;
            if shift == 63 && !matches!(byte, 0 | 0x7f) {
                return Err(error("fixup SLEB128 overflow"));
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                if byte & 0x40 != 0 && shift < 57 {
                    value |= u64::MAX << (shift + 7);
                }
                return Ok(value as i64);
            }
        }
        Err(error("fixup SLEB128 overflow"))
    }
    fn name(&mut self) -> Result<String, LinkError> {
        let end = self.bytes[self.index..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|length| self.index + length)
            .ok_or_else(|| error("unterminated fixup symbol"))?;
        let name = std::str::from_utf8(&self.bytes[self.index..end])
            .map_err(error)?
            .to_owned();
        self.index = end + 1;
        Ok(name)
    }
}
