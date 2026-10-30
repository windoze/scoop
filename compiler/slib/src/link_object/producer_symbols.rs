//! Remove production-only end markers after object verification has completed.
//!
//! Section identities and contents stay intact; only symbol references and file
//! offsets change. In particular, start markers may still own relocations.

use std::fmt;

mod coff;
mod elf;
mod macho;

#[derive(Debug)]
pub struct ProducerSymbolError(String);

impl fmt::Display for ProducerSymbolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot remove production symbols: {}", self.0)
    }
}

impl std::error::Error for ProducerSymbolError {}

/// Called only for verified compiler/bridge objects, after range consumers.
pub fn remove_production_end_symbols(bytes: &[u8]) -> Result<Vec<u8>> {
    match object::FileKind::parse(bytes).map_err(error)? {
        object::FileKind::MachO64 => macho::strip(bytes),
        object::FileKind::Elf64 => elf::strip(bytes),
        object::FileKind::Coff => coff::strip(bytes, false),
        object::FileKind::CoffBig => coff::strip(bytes, true),
        _ => Err(error("unsupported production object format")),
    }
}

type Result<T> = std::result::Result<T, ProducerSymbolError>;

fn error(message: impl fmt::Display) -> ProducerSymbolError {
    ProducerSymbolError(message.to_string())
}

fn is_end(name: &[u8]) -> bool {
    let name = name.strip_prefix(b"_").unwrap_or(name);
    name.strip_prefix(b"scoop$1$be$")
        .is_some_and(|id| id.len() == 64 && id.iter().all(u8::is_ascii_hexdigit))
}

fn slice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    bytes
        .get(
            offset
                ..offset
                    .checked_add(length)
                    .ok_or_else(|| error("offset overflow"))?,
        )
        .ok_or_else(|| error("truncated object"))
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        slice(bytes, offset, 2)?.try_into().unwrap(),
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        slice(bytes, offset, 4)?.try_into().unwrap(),
    ))
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        slice(bytes, offset, 8)?.try_into().unwrap(),
    ))
}

fn put32(bytes: &mut [u8], offset: usize, value: u32) -> Result<()> {
    let field = bytes
        .get_mut(
            offset
                ..offset
                    .checked_add(4)
                    .ok_or_else(|| error("offset overflow"))?,
        )
        .ok_or_else(|| error("truncated object field"))?;
    field.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn put64(bytes: &mut [u8], offset: usize, value: u64) -> Result<()> {
    let field = bytes
        .get_mut(
            offset
                ..offset
                    .checked_add(8)
                    .ok_or_else(|| error("offset overflow"))?,
        )
        .ok_or_else(|| error("truncated object field"))?;
    field.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn narrow(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(error)
}

fn name(strings: &[u8], offset: u32) -> Result<&[u8]> {
    let bytes = strings
        .get(offset as usize..)
        .ok_or_else(|| error("string offset out of range"))?;
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| error("unterminated object name"))?;
    Ok(&bytes[..end])
}

fn add_name(strings: &mut Vec<u8>, value: &[u8]) -> Result<u32> {
    if value.is_empty() && strings.first() == Some(&0) {
        return Ok(0);
    }
    let offset = narrow(strings.len())?;
    strings.extend_from_slice(value);
    strings.push(0);
    Ok(offset)
}

fn remap(indices: &[Option<u32>], index: u32) -> Result<u32> {
    indices
        .get(index as usize)
        .ok_or_else(|| error("symbol index out of range"))?
        .ok_or_else(|| error("production end symbol is still referenced"))
}

fn append_aligned(output: &mut Vec<u8>, bytes: &[u8], alignment: usize) -> Result<usize> {
    let offset = output
        .len()
        .checked_next_multiple_of(alignment.max(1))
        .ok_or_else(|| error("section alignment overflow"))?;
    output.resize(offset, 0);
    output.extend_from_slice(bytes);
    Ok(offset)
}
