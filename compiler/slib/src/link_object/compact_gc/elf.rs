use object::elf;

use super::*;

pub(super) fn replace(input: &[u8], map: &[u8]) -> Result<Vec<u8>, CompactGcMapError> {
    let count = u16_at(input, 60)? as usize;
    let string_index = u16_at(input, 62)? as usize;
    let mut headers = span(input, u64_at(input, 40)?, count as u64 * 64)?.to_vec();
    let string_header = headers
        .get(string_index * 64..(string_index + 1) * 64)
        .ok_or_else(|| error("invalid ELF section name table"))?;
    let mut strings = span(
        input,
        u64_at(string_header, 24)?,
        u64_at(string_header, 32)?,
    )?
    .to_vec();
    let mut gc = None;
    for (index, header) in headers.as_chunks::<64>().0.iter().enumerate() {
        let offset = u32_at(header, 0)? as usize;
        let tail = strings
            .get(offset..)
            .ok_or_else(|| error("invalid ELF section name"))?;
        if tail.starts_with(b".llvm_stackmaps\0") {
            gc = Some(index);
        }
    }
    let gc = gc.ok_or_else(|| error("missing temporary LLVM stackmap section"))?;
    let old_size = u64_at(&headers, gc * 64 + 32)?;
    let gc_name = add_name(&mut strings, b".rodata.scoop.gc")?;
    put(&mut headers, gc * 64, gc_name.to_le_bytes())?;
    // Keep SHF_GROUP and its existing group membership, but the new map has no
    // relocated pointers and belongs in read-only data.
    let flags = u64_at(&headers, gc * 64 + 8)? & !u64::from(elf::SHF_WRITE);
    put(&mut headers, gc * 64 + 8, flags.to_le_bytes())?;
    for header in headers.as_chunks_mut::<64>().0.iter_mut() {
        if matches!(u32_at(header, 4)?, elf::SHT_RELA | elf::SHT_REL)
            && u32_at(header, 44)? as usize == gc
        {
            let name = if u32_at(header, 4)? == elf::SHT_RELA {
                b".rela.rodata.scoop.gc".as_slice()
            } else {
                b".rel.rodata.scoop.gc".as_slice()
            };
            put(header, 0, add_name(&mut strings, name)?.to_le_bytes())?;
        }
    }
    let mut output = span(input, 0, 64)?.to_vec();
    for (index, header) in headers.as_chunks_mut::<64>().0.iter_mut().enumerate() {
        let kind = u32_at(header, 4)?;
        if kind == elf::SHT_NULL || kind == elf::SHT_NOBITS {
            continue;
        }
        let mut data = if index == gc {
            map.to_vec()
        } else if index == string_index {
            strings.clone()
        } else if matches!(kind, elf::SHT_RELA | elf::SHT_REL) && u32_at(header, 44)? as usize == gc
        {
            Vec::new()
        } else {
            span(input, u64_at(header, 24)?, u64_at(header, 32)?)?.to_vec()
        };
        if kind == elf::SHT_SYMTAB {
            for symbol in data.as_chunks_mut::<24>().0.iter_mut() {
                if u16_at(symbol, 6)? as usize == gc {
                    let value = boundary_value(u64_at(symbol, 8)?, 0, old_size, map.len())?;
                    put(symbol, 8, value.to_le_bytes())?;
                    if u64_at(symbol, 16)? == old_size {
                        put(symbol, 16, (map.len() as u64).to_le_bytes())?;
                    }
                }
            }
        }
        let offset = append(&mut output, &data, u64_at(header, 48)?)?;
        put(header, 24, offset.to_le_bytes())?;
        put(header, 32, (data.len() as u64).to_le_bytes())?;
    }
    let offset = append(&mut output, &headers, 8)?;
    put(&mut output, 40, offset.to_le_bytes())?;
    Ok(output)
}

fn add_name(strings: &mut Vec<u8>, name: &[u8]) -> Result<u32, CompactGcMapError> {
    let offset = u32::try_from(strings.len()).map_err(error)?;
    strings.extend_from_slice(name);
    strings.push(0);
    Ok(offset)
}
