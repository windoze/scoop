use super::*;
use object::macho;

pub(super) enum Selection {
    ProductionEnds,
    EmptyCodeLabels,
}

pub(super) fn strip(bytes: &[u8], selection: Selection) -> Result<Vec<u8>> {
    if u32_at(bytes, 0)? != macho::MH_MAGIC_64 || u32_at(bytes, 12)? != macho::MH_OBJECT {
        return Err(error("expected little-endian Mach-O object"));
    }
    let mut command = 32;
    let mut symtab = None;
    let mut dysymtab = None;
    let mut relocations = Vec::new();
    let mut empty_code_sections = vec![false];
    for _ in 0..u32_at(bytes, 16)? {
        let size = u32_at(bytes, command + 4)? as usize;
        slice(bytes, command, size)?;
        match u32_at(bytes, command)? {
            macho::LC_SYMTAB => symtab = Some(command),
            macho::LC_DYSYMTAB => dysymtab = Some(command),
            macho::LC_SEGMENT_64 => {
                for i in 0..u32_at(bytes, command + 64)? as usize {
                    let section = command + 72 + i * 80;
                    relocations.push((u32_at(bytes, section + 56)?, u32_at(bytes, section + 60)?));
                    empty_code_sections.push(
                        u64_at(bytes, section + 40)? == 0
                            && u32_at(bytes, section + 64)?
                                & (macho::S_ATTR_PURE_INSTRUCTIONS
                                    | macho::S_ATTR_SOME_INSTRUCTIONS)
                                != 0,
                    );
                }
            }
            _ => {}
        }
        if size < 8 {
            return Err(error("invalid Mach-O load command"));
        }
        command += size;
    }
    if matches!(selection, Selection::EmptyCodeLabels)
        && (!empty_code_sections.iter().any(|empty| *empty) || symtab.is_none())
    {
        return Ok(bytes.to_vec());
    }
    let symtab = symtab.ok_or_else(|| error("missing Mach-O symbol table"))?;
    let start = u32_at(bytes, symtab + 8)? as usize;
    let count = u32_at(bytes, symtab + 12)? as usize;
    let strings_offset = u32_at(bytes, symtab + 16)? as usize;
    let old_strings = slice(bytes, strings_offset, u32_at(bytes, symtab + 20)? as usize)?;
    let symbols = slice(bytes, start, count * 16)?;
    // LLVM's relocatable objects place both tables last. Other native producers
    // may keep additional data after them; retain it and append the new tables.
    let tables_last = start + symbols.len() <= strings_offset
        && strings_offset + old_strings.len() == bytes.len();
    if matches!(selection, Selection::ProductionEnds) && !tables_last {
        return Err(error("unexpected Mach-O symbol table placement"));
    }
    let referenced = if matches!(selection, Selection::EmptyCodeLabels) {
        referenced_symbols(bytes, &relocations, dysymtab)?
    } else {
        std::collections::BTreeSet::new()
    };
    let mut indices = Vec::with_capacity(count);
    let mut prefix = vec![0u32];
    let mut strings = vec![0];
    let mut kept = Vec::new();
    for (index, symbol) in symbols.as_chunks::<16>().0.iter().enumerate() {
        let symbol_name = name(old_strings, u32_at(symbol, 0)?)?;
        let remove = match selection {
            Selection::ProductionEnds => is_end(symbol_name),
            Selection::EmptyCodeLabels => {
                symbol[4] & (macho::N_STAB | macho::N_TYPE | macho::N_EXT) == macho::N_SECT
                    && empty_code_sections
                        .get(usize::from(symbol[5]))
                        .copied()
                        .unwrap_or(false)
                    && !referenced.contains(&narrow(index)?)
            }
        };
        if remove {
            indices.push(None);
        } else {
            indices.push(Some(narrow(kept.len() / 16)?));
            let mut symbol = symbol.to_vec();
            put32(&mut symbol, 0, add_name(&mut strings, symbol_name)?)?;
            if symbol[4] & macho::N_TYPE == macho::N_INDR {
                let target = u32::try_from(u64_at(&symbol, 8)?).map_err(error)?;
                put64(
                    &mut symbol,
                    8,
                    u64::from(add_name(&mut strings, name(old_strings, target)?)?),
                )?;
            }
            kept.extend(symbol);
        }
        prefix.push(narrow(kept.len() / 16)?);
    }
    if kept.len() == symbols.len() {
        return Ok(bytes.to_vec());
    }
    let mut output = if tables_last {
        bytes[..start].to_vec()
    } else {
        bytes.to_vec()
    };
    if let Some(dysymtab) = dysymtab {
        for field in [8, 16, 24] {
            let begin = u32_at(bytes, dysymtab + field)? as usize;
            let end = begin + u32_at(bytes, dysymtab + field + 4)? as usize;
            let new_begin = *prefix
                .get(begin)
                .ok_or_else(|| error("invalid Mach-O symbol partition"))?;
            let new_end = *prefix
                .get(end)
                .ok_or_else(|| error("invalid Mach-O symbol partition"))?;
            put32(&mut output, dysymtab + field, new_begin)?;
            put32(&mut output, dysymtab + field + 4, new_end - new_begin)?;
        }
        // Module and TOC tables belong to linked images, not MH_OBJECT.
        if u32_at(bytes, dysymtab + 36)? != 0
            || u32_at(bytes, dysymtab + 44)? != 0
            || u32_at(bytes, dysymtab + 52)? != 0
        {
            return Err(error("unexpected Mach-O dynamic module table"));
        }
        let offset = u32_at(bytes, dysymtab + 56)? as usize;
        for i in 0..u32_at(bytes, dysymtab + 60)? as usize {
            let field = offset + i * 4;
            let index = u32_at(bytes, field)?;
            if index & (macho::INDIRECT_SYMBOL_LOCAL | macho::INDIRECT_SYMBOL_ABS) == 0 {
                put32(&mut output, field, remap(&indices, index)?)?;
            }
        }
        for field in [64, 72] {
            relocations.push((
                u32_at(bytes, dysymtab + field)?,
                u32_at(bytes, dysymtab + field + 4)?,
            ));
        }
    }
    for (offset, count) in relocations {
        for i in 0..count as usize {
            let field = offset as usize + i * 8;
            if u32_at(bytes, field)? & macho::R_SCATTERED != 0 {
                return Err(error("unexpected scattered Mach-O relocation"));
            }
            let info = u32_at(bytes, field + 4)?;
            if info & (1 << 27) != 0 {
                let index = remap(&indices, info & 0x00ff_ffff)?;
                put32(&mut output, field + 4, (info & 0xff00_0000) | index)?;
            }
        }
    }
    let symbols_offset = append_aligned(&mut output, &kept, 8)?;
    put32(&mut output, symtab + 8, narrow(symbols_offset)?)?;
    put32(&mut output, symtab + 12, *prefix.last().unwrap())?;
    let strings_offset = narrow(output.len())?;
    put32(&mut output, symtab + 16, strings_offset)?;
    strings.resize(strings.len().next_multiple_of(8), 0);
    put32(&mut output, symtab + 20, narrow(strings.len())?)?;
    output.extend(strings);
    Ok(output)
}

fn referenced_symbols(
    bytes: &[u8],
    section_relocations: &[(u32, u32)],
    dysymtab: Option<usize>,
) -> Result<std::collections::BTreeSet<u32>> {
    let mut referenced = std::collections::BTreeSet::new();
    let mut relocations = section_relocations.to_vec();
    if let Some(command) = dysymtab {
        let offset = u32_at(bytes, command + 56)? as usize;
        for i in 0..u32_at(bytes, command + 60)? as usize {
            let index = u32_at(bytes, offset + i * 4)?;
            if index & (macho::INDIRECT_SYMBOL_LOCAL | macho::INDIRECT_SYMBOL_ABS) == 0 {
                referenced.insert(index);
            }
        }
        for field in [64, 72] {
            relocations.push((
                u32_at(bytes, command + field)?,
                u32_at(bytes, command + field + 4)?,
            ));
        }
    }
    for (offset, count) in relocations {
        for i in 0..count as usize {
            let field = offset as usize + i * 8;
            let info = u32_at(bytes, field + 4)?;
            if u32_at(bytes, field)? & macho::R_SCATTERED == 0 && info & (1 << 27) != 0 {
                referenced.insert(info & 0x00ff_ffff);
            }
        }
    }
    Ok(referenced)
}
