//! Structural checks for ld's ad-hoc signature; no publisher authentication.
use crate::{LinkError, error};

pub(super) fn check(bytes: &[u8], signed_length: u32) -> Result<(), LinkError> {
    if word(bytes, 0)? != 0xfade0cc0 {
        return Err(error("invalid code signature superblob"));
    }
    let length = word(bytes, 4)? as usize;
    let bytes = bytes
        .get(..length)
        .ok_or_else(|| error("truncated code signature superblob"))?;
    let count = word(bytes, 8)? as usize;
    let table_end = count
        .checked_mul(8)
        .and_then(|size| size.checked_add(12))
        .ok_or_else(|| error("code signature index overflow"))?;
    if count != 1 || table_end > bytes.len() || word(bytes, 12)? != 0 {
        return Err(error(
            "ad-hoc signature must contain one primary CodeDirectory",
        ));
    }
    let offset = word(bytes, 16)? as usize;
    if offset < table_end {
        return Err(error("code signature index overlaps its blob"));
    }
    let directory = bytes
        .get(offset..)
        .ok_or_else(|| error("missing CodeDirectory"))?;
    let length = word(directory, 4)? as usize;
    let directory = directory
        .get(..length)
        .ok_or_else(|| error("truncated CodeDirectory"))?;
    if word(directory, 0)? != 0xfade0c02
        || word(directory, 8)? < 0x20100
        || word(directory, 12)? & 2 == 0
    {
        return Err(error("invalid ad-hoc CodeDirectory header"));
    }
    let hash_offset = word(directory, 16)? as usize;
    let identifier = word(directory, 20)? as usize;
    let special = word(directory, 24)? as usize;
    let pages = word(directory, 28)? as usize;
    let code_limit = word(directory, 32)?;
    let hash = directory
        .get(36..40)
        .ok_or_else(|| error("missing CodeDirectory hash format"))?;
    if hash[0] != 32 || hash[1] != 2 || !(12..=16).contains(&hash[3]) || code_limit != signed_length
    {
        return Err(error(
            "CodeDirectory hash format or signed range differs from the executable",
        ));
    }
    let page_size = 1usize << hash[3];
    let hashes_end = pages
        .checked_mul(32)
        .and_then(|size| hash_offset.checked_add(size));
    let hashes_start = special
        .checked_mul(32)
        .and_then(|size| hash_offset.checked_sub(size));
    if pages != (code_limit as usize).div_ceil(page_size)
        || hashes_end.is_none_or(|end| end > directory.len())
        || hashes_start.is_none_or(|start| start < 44)
        || identifier < 44
        || identifier >= hashes_start.unwrap_or(0)
        || directory[identifier..hashes_start.unwrap_or(0)]
            .split(|byte| *byte == 0)
            .next()
            != Some(b"program")
    {
        return Err(error("CodeDirectory identifier or page table is invalid"));
    }
    Ok(())
}

fn word(bytes: &[u8], offset: usize) -> Result<u32, LinkError> {
    Ok(u32::from_be_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or_else(|| error("truncated code signature field"))?
            .try_into()
            .map_err(error)?,
    ))
}
