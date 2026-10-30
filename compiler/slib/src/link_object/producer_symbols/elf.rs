use super::*;
use object::elf;

// LLVM Extensions: address-significance table (ULEB128 symbol indices).
const SHT_LLVM_ADDRSIG: u32 = 0x6fff4c03;

pub(super) fn strip(bytes: &[u8]) -> Result<Vec<u8>> {
    if slice(bytes, 4, 2)? != [2, 1]
        || u16_at(bytes, 16)? != elf::ET_REL
        || u16_at(bytes, 56)? != 0
        || u16_at(bytes, 58)? != 64
    {
        return Err(error("expected ELF64LE relocatable object"));
    }
    let offset = usize::try_from(u64_at(bytes, 40)?).map_err(error)?;
    let count = match u16_at(bytes, 60)? {
        0 => usize::try_from(u64_at(bytes, offset + 32)?).map_err(error)?,
        count => usize::from(count),
    };
    let mut headers = slice(
        bytes,
        offset,
        count
            .checked_mul(64)
            .ok_or_else(|| error("section count overflow"))?,
    )?
    .chunks_exact(64)
    .map(<[u8]>::to_vec)
    .collect::<Vec<_>>();
    let mut sections = headers
        .iter()
        .map(|header| {
            if matches!(u32_at(header, 4)?, elf::SHT_NULL | elf::SHT_NOBITS) {
                return Ok(Vec::new());
            }
            Ok(slice(
                bytes,
                usize::try_from(u64_at(header, 24)?).map_err(error)?,
                usize::try_from(u64_at(header, 32)?).map_err(error)?,
            )?
            .to_vec())
        })
        .collect::<Result<Vec<_>>>()?;
    let tables = headers
        .iter()
        .enumerate()
        .filter(|(_, h)| u32_at(h, 4).ok() == Some(elf::SHT_SYMTAB))
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    let [symtab] = tables.as_slice() else {
        return Err(error("expected one ELF symbol table"));
    };
    let symtab = *symtab;
    if u64_at(&headers[symtab], 56)? != 24 || sections[symtab].len() % 24 != 0 {
        return Err(error("invalid ELF symbol table"));
    }
    let strtab = u32_at(&headers[symtab], 40)? as usize;
    let old_strings = sections
        .get(strtab)
        .ok_or_else(|| error("invalid ELF string table"))?;
    let mut indices = Vec::new();
    let mut kept = Vec::new();
    let mut strings = vec![0];
    let local_count = u32_at(&headers[symtab], 44)? as usize;
    let mut new_local_count = 0;
    for (index, symbol) in sections[symtab].chunks_exact(24).enumerate() {
        let symbol_name = name(old_strings, u32_at(symbol, 0)?)?;
        if is_end(symbol_name) {
            indices.push(None);
        } else {
            indices.push(Some(narrow(kept.len() / 24)?));
            let mut symbol = symbol.to_vec();
            put32(&mut symbol, 0, add_name(&mut strings, symbol_name)?)?;
            kept.extend(symbol);
            if index < local_count {
                new_local_count += 1;
            }
        }
    }
    if kept.len() == sections[symtab].len() {
        return Ok(bytes.to_vec());
    }
    let shstrtab = match u16_at(bytes, 62)? {
        elf::SHN_XINDEX => u32_at(&headers[0], 40)? as usize,
        index => usize::from(index),
    };
    if shstrtab == strtab {
        for header in &mut headers {
            let new_name = add_name(&mut strings, name(old_strings, u32_at(header, 0)?)?)?;
            put32(header, 0, new_name)?;
        }
    }
    sections[symtab] = kept;
    sections[strtab] = strings;
    put32(&mut headers[symtab], 44, new_local_count)?;
    for (header, data) in headers.iter_mut().zip(&mut sections) {
        if u32_at(header, 40)? as usize != symtab {
            continue;
        }
        match u32_at(header, 4)? {
            elf::SHT_REL | elf::SHT_RELA => {
                let width = if u32_at(header, 4)? == elf::SHT_REL {
                    16
                } else {
                    24
                };
                if data.len() % width != 0 {
                    return Err(error("invalid ELF relocation table"));
                }
                for relocation in data.chunks_exact_mut(width) {
                    let info = u64_at(relocation, 8)?;
                    put64(
                        relocation,
                        8,
                        (u64::from(remap(&indices, (info >> 32) as u32)?) << 32)
                            | (info & 0xffff_ffff),
                    )?;
                }
            }
            elf::SHT_GROUP => {
                let signature = remap(&indices, u32_at(header, 44)?)?;
                put32(header, 44, signature)?;
            }
            elf::SHT_SYMTAB_SHNDX => {
                if data.len() != indices.len() * 4 {
                    return Err(error("invalid ELF extended symbol indices"));
                }
                *data = data
                    .chunks_exact(4)
                    .zip(&indices)
                    .filter(|(_, index)| index.is_some())
                    .flat_map(|(entry, _)| entry.iter().copied())
                    .collect();
            }
            SHT_LLVM_ADDRSIG => *data = remap_addrsig(data, &indices)?,
            _ => {}
        }
    }
    // Repack payloads to discard superseded tables left by producer-side edits.
    let mut output = slice(bytes, 0, 64)?.to_vec();
    for (header, data) in headers.iter_mut().zip(&sections).skip(1) {
        let alignment = usize::try_from(u64_at(header, 48)?).map_err(error)?;
        let offset = append_aligned(&mut output, data, alignment)?;
        put64(header, 24, offset as u64)?;
        if u32_at(header, 4)? != elf::SHT_NOBITS {
            put64(header, 32, data.len() as u64)?;
        }
    }
    let offset = append_aligned(&mut output, &[], 8)?;
    put64(&mut output, 40, offset as u64)?;
    for header in headers {
        output.extend(header);
    }
    Ok(output)
}

fn remap_addrsig(data: &[u8], indices: &[Option<u32>]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut value = 0u32;
    let mut shift = 0;
    for byte in data {
        if shift > 28 || (shift == 28 && byte & 0x70 != 0) {
            return Err(error("ELF address-significance index overflow"));
        }
        value |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 != 0 {
            shift += 7;
            continue;
        }
        value = remap(indices, value)?;
        while value >= 128 {
            output.push((value as u8 & 0x7f) | 0x80);
            value >>= 7;
        }
        output.push(value as u8);
        value = 0;
        shift = 0;
    }
    if shift != 0 {
        return Err(error("truncated ELF address-significance index"));
    }
    Ok(output)
}
