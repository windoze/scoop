use super::*;
use object::{Object, ObjectSection, pe};

pub(super) fn strip(bytes: &[u8], big: bool) -> Result<Vec<u8>> {
    let (width, pointer, count_field, header_size, section_count) = if big {
        (20, 48, 52, 56, u32_at(bytes, 44)? as usize)
    } else {
        (
            18,
            8,
            12,
            20 + usize::from(u16_at(bytes, 16)?),
            usize::from(u16_at(bytes, 2)?),
        )
    };
    let count = u32_at(bytes, count_field)? as usize;
    let start = u32_at(bytes, pointer)? as usize;
    let old_symbols = slice(
        bytes,
        start,
        count
            .checked_mul(width)
            .ok_or_else(|| error("COFF symbol count overflow"))?,
    )?;
    let strings_offset = start + old_symbols.len();
    let old_strings = slice(
        bytes,
        strings_offset,
        u32_at(bytes, strings_offset)? as usize,
    )?;
    let mut strings = vec![0; 4];
    let mut symbols = Vec::new();
    let mut indices = vec![None; count];
    let mut index = 0;
    while index < count {
        let entry = slice(old_symbols, index * width, width)?;
        let aux_count = usize::from(entry[width - 1]);
        let entries = slice(old_symbols, index * width, (aux_count + 1) * width)?;
        let symbol_name = if u32_at(entry, 0)? == 0 {
            name(old_strings, u32_at(entry, 4)?)?
        } else {
            let length = entry[..8].iter().position(|b| *b == 0).unwrap_or(8);
            &entry[..length]
        };
        if !is_end(symbol_name) {
            let new_index = narrow(symbols.len() / width)?;
            for i in 0..=aux_count {
                indices[index + i] = Some(new_index + narrow(i)?);
            }
            let mut entries = entries.to_vec();
            if u32_at(entry, 0)? == 0 {
                put32(&mut entries, 4, add_name(&mut strings, symbol_name)?)?;
            }
            symbols.extend(entries);
        }
        index += aux_count + 1;
    }
    if symbols.len() == old_symbols.len() {
        return Ok(bytes.to_vec());
    }
    index = 0;
    while index < symbols.len() / width {
        let offset = index * width;
        let class = symbols[offset + width - 2];
        let aux_count = usize::from(symbols[offset + width - 1]);
        if aux_count != 0 {
            let function = u16_at(&symbols, offset + width - 4)? & 0x30 == 0x20;
            if class == pe::IMAGE_SYM_CLASS_WEAK_EXTERNAL {
                remap_field(&mut symbols, offset + width, &indices, false)?;
            } else if class == pe::IMAGE_SYM_CLASS_EXTERNAL && function {
                remap_field(&mut symbols, offset + width, &indices, true)?;
                remap_field(&mut symbols, offset + width + 12, &indices, true)?;
            } else if class == pe::IMAGE_SYM_CLASS_FUNCTION {
                remap_field(&mut symbols, offset + width + 12, &indices, true)?;
            }
        }
        index += aux_count + 1;
    }
    let file = object::File::parse(bytes).map_err(error)?;
    let names = file
        .sections()
        .map(|s| s.name_bytes().map_err(error))
        .collect::<Result<Vec<_>>>()?;
    let mut output = slice(bytes, 0, header_size + section_count * 40)?.to_vec();
    let mut line_ranges = Vec::new();
    for i in 0..section_count {
        let header = header_size + i * 40;
        if bytes[header] == b'/' {
            let offset = add_name(
                &mut strings,
                names
                    .get(i)
                    .ok_or_else(|| error("missing COFF section name"))?,
            )?;
            let encoded = section_name_offset(offset);
            output[header..header + 8].fill(0);
            output[header..header + encoded.len()].copy_from_slice(encoded.as_bytes());
        }
        let data_offset = u32_at(bytes, header + 20)? as usize;
        let data_size = u32_at(bytes, header + 16)? as usize;
        if data_offset != 0 {
            // COFF's in-memory alignment lives in section characteristics;
            // file payloads need not add padding between small sections.
            let offset = append_aligned(&mut output, slice(bytes, data_offset, data_size)?, 1)?;
            put32(&mut output, header + 20, narrow(offset)?)?;
        }
        let relocation_offset = u32_at(bytes, header + 24)? as usize;
        let mut relocation_count = usize::from(u16_at(bytes, header + 32)?);
        let overflow = u32_at(bytes, header + 36)? & pe::IMAGE_SCN_LNK_NRELOC_OVFL != 0;
        if overflow {
            relocation_count = u32_at(bytes, relocation_offset)? as usize;
        }
        if relocation_count != 0 {
            let mut relocations = slice(bytes, relocation_offset, relocation_count * 10)?.to_vec();
            for relocation in relocations.chunks_exact_mut(10).skip(usize::from(overflow)) {
                remap_field(relocation, 4, &indices, false)?;
            }
            let offset = append_aligned(&mut output, &relocations, 4)?;
            put32(&mut output, header + 24, narrow(offset)?)?;
        }
        let line_count = usize::from(u16_at(bytes, header + 34)?);
        if line_count != 0 {
            let old_offset = u32_at(bytes, header + 28)? as usize;
            let mut lines = slice(bytes, old_offset, line_count * 6)?.to_vec();
            for line in lines.chunks_exact_mut(6) {
                if u16_at(line, 4)? == 0 {
                    remap_field(line, 0, &indices, false)?;
                }
            }
            let offset = append_aligned(&mut output, &lines, 2)?;
            put32(&mut output, header + 28, narrow(offset)?)?;
            line_ranges.push((old_offset..old_offset + lines.len(), offset));
        }
    }
    index = 0;
    while index < symbols.len() / width {
        let offset = index * width;
        let aux_count = usize::from(symbols[offset + width - 1]);
        if aux_count != 0
            && symbols[offset + width - 2] == pe::IMAGE_SYM_CLASS_EXTERNAL
            && u16_at(&symbols, offset + width - 4)? & 0x30 == 0x20
        {
            let field = offset + width + 8;
            let old = u32_at(&symbols, field)? as usize;
            if old != 0 {
                let (range, new) = line_ranges
                    .iter()
                    .find(|(range, _)| range.contains(&old))
                    .ok_or_else(|| error("COFF function line pointer out of range"))?;
                put32(&mut symbols, field, narrow(new + old - range.start)?)?;
            }
        }
        index += aux_count + 1;
    }
    let offset = append_aligned(&mut output, &symbols, 4)?;
    put32(&mut output, pointer, narrow(offset)?)?;
    put32(&mut output, count_field, narrow(symbols.len() / width)?)?;
    let length = narrow(strings.len())?;
    put32(&mut strings, 0, length)?;
    output.extend(strings);
    Ok(output)
}

fn section_name_offset(offset: u32) -> String {
    let decimal = format!("/{offset}");
    if decimal.len() <= 8 {
        return decimal;
    }
    // LLVM/GNU COFF's six-digit base64 form for large string tables.
    const DIGITS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::from("//");
    for shift in [30, 24, 18, 12, 6, 0] {
        encoded.push(char::from(DIGITS[((offset >> shift) & 63) as usize]));
    }
    encoded
}

fn remap_field(
    bytes: &mut [u8],
    field: usize,
    indices: &[Option<u32>],
    zero_is_null: bool,
) -> Result<()> {
    let index = u32_at(bytes, field)?;
    if !zero_is_null || index != 0 {
        put32(bytes, field, remap(indices, index)?)?;
    }
    Ok(())
}
