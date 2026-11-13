use object::macho;

use super::*;

pub(super) fn replace(input: &[u8], map: &[u8]) -> Result<Vec<u8>, CompactGcMapError> {
    let commands_end = 32usize
        .checked_add(u32_at(input, 20)? as usize)
        .ok_or_else(|| error("Mach-O commands overflow"))?;
    let mut output = span(input, 0, commands_end as u64)?.to_vec();
    let mut cursor = 32;
    let mut sections = Vec::new();
    let mut segment = None;
    let mut symtab = None;
    let mut linkedit = Vec::new();
    for _ in 0..u32_at(input, 16)? {
        match u32_at(input, cursor)? {
            macho::LC_SEGMENT_64 => {
                segment = Some(cursor);
                for index in 0..u32_at(input, cursor + 64)? as usize {
                    sections.push(cursor + 72 + index * 80);
                }
            }
            macho::LC_SYMTAB => symtab = Some(cursor),
            macho::LC_DATA_IN_CODE | macho::LC_LINKER_OPTIMIZATION_HINT => {
                linkedit.push(cursor);
            }
            _ => {}
        }
        cursor += u32_at(input, cursor + 4)? as usize;
    }
    let segment = segment.ok_or_else(|| error("missing Mach-O segment"))?;
    let symtab = symtab.ok_or_else(|| error("missing Mach-O symbols"))?;
    let gc = sections
        .iter()
        .position(|offset| {
            input
                .get(*offset..*offset + 16)
                .is_some_and(|name| name == b"__llvm_stackmaps")
        })
        .ok_or_else(|| error("missing temporary LLVM stackmap section"))?;
    let gc_header = sections[gc];
    let gc_start = u64_at(input, gc_header + 32)?;
    let gc_size = u64_at(input, gc_header + 40)?;
    let data_start = output.len() as u64;
    for (index, header) in sections.iter().copied().enumerate() {
        let flags = u32_at(input, header + 64)?;
        if matches!(
            flags & macho::SECTION_TYPE,
            macho::S_ZEROFILL | macho::S_THREAD_LOCAL_ZEROFILL | macho::S_GB_ZEROFILL
        ) {
            continue;
        }
        let data = if index == gc {
            map
        } else {
            span(
                input,
                u32_at(input, header + 48)? as u64,
                u64_at(input, header + 40)?,
            )?
        };
        let alignment = 1u64
            .checked_shl(u32_at(input, header + 52)?)
            .ok_or_else(|| error("Mach-O alignment overflows"))?;
        let offset = append(&mut output, data, alignment)?;
        put(
            &mut output,
            header + 48,
            u32::try_from(offset).map_err(error)?.to_le_bytes(),
        )?;
        if index == gc {
            output[header..header + 32].fill(0);
            output[header..header + 10].copy_from_slice(b"__scoop_gc");
            output[header + 16..header + 28].copy_from_slice(b"__DATA_CONST");
            put(&mut output, header + 40, (map.len() as u64).to_le_bytes())?;
        }
    }
    let data_size = output.len() as u64 - data_start;
    put(&mut output, segment + 40, data_start.to_le_bytes())?;
    put(&mut output, segment + 48, data_size.to_le_bytes())?;
    for (index, header) in sections.iter().copied().enumerate() {
        let count = u32_at(input, header + 60)?;
        if index == gc || count == 0 {
            put(&mut output, header + 56, 0u32.to_le_bytes())?;
            put(&mut output, header + 60, 0u32.to_le_bytes())?;
            continue;
        }
        let data = span(input, u32_at(input, header + 56)? as u64, count as u64 * 8)?;
        let offset = append(&mut output, data, 8)?;
        put(
            &mut output,
            header + 56,
            u32::try_from(offset).map_err(error)?.to_le_bytes(),
        )?;
    }
    // LLVM emits these outside the sections. Their MH_OBJECT entries refer to
    // unchanged section addresses, while their load-command file offsets move.
    for command in linkedit {
        let data = span(
            input,
            u32_at(input, command + 8)? as u64,
            u32_at(input, command + 12)? as u64,
        )?;
        let offset = append(&mut output, data, 8)?;
        put(
            &mut output,
            command + 8,
            u32::try_from(offset).map_err(error)?.to_le_bytes(),
        )?;
    }
    let count = u32_at(input, symtab + 12)? as usize;
    let mut symbols = span(input, u32_at(input, symtab + 8)? as u64, count as u64 * 16)?.to_vec();
    for symbol in symbols.as_chunks_mut::<16>().0.iter_mut() {
        if symbol[4] & macho::N_TYPE == macho::N_SECT && symbol[5] as usize == gc + 1 {
            let value = boundary_value(u64_at(symbol, 8)?, gc_start, gc_size, map.len())?;
            put(symbol, 8, value.to_le_bytes())?;
        }
    }
    let offset = append(&mut output, &symbols, 8)?;
    put(
        &mut output,
        symtab + 8,
        u32::try_from(offset).map_err(error)?.to_le_bytes(),
    )?;
    let strings = span(
        input,
        u32_at(input, symtab + 16)? as u64,
        u32_at(input, symtab + 20)? as u64,
    )?;
    let offset = append(&mut output, strings, 1)?;
    put(
        &mut output,
        symtab + 16,
        u32::try_from(offset).map_err(error)?.to_le_bytes(),
    )?;
    Ok(output)
}
