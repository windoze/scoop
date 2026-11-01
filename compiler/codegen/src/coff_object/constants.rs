//! Keep LLVM constant pools in the same selection unit as their callable.

use object::{RelocationFlags, RelocationTarget, SectionFlags};

use super::*;

pub(crate) fn merge_callable_constants(bytes: &mut Vec<u8>) -> Result<(), CodegenError> {
    let mut output = CoffObject::read(bytes)?;
    let mut sections = output
        .file
        .sections()
        .filter(|section| {
            section.size() != 0
                && section
                    .name()
                    .is_ok_and(|name| name == ".rdata" || name.starts_with(".rdata$.refptr."))
        })
        .map(|section| {
            let index = section.index();
            output
                .section_aux
                .get(&index.0)
                .ok_or_else(|| error("constant section has no auxiliary record"))?;
            Ok(index)
        })
        .collect::<Result<Vec<_>, CodegenError>>()?;
    // LLD resolves associative contributions in symbol-table order. A moved
    // section label must follow the retained section definition; LLVM's section
    // headers can have a different order, notably for MinGW refptrs and tables.
    sections.sort_by_key(|index| output.section_aux[&index.0]);
    let Some(&primary) = sections.first() else {
        return Ok(());
    };
    let mut bases = BTreeMap::new();
    let mut data = Vec::new();
    let mut relocations = Vec::new();
    let mut alignment = 1;
    for &index in &sections {
        let section = output.file.section_by_index(index).map_err(error)?;
        let align = usize::try_from(section.align()).map_err(error)?;
        alignment = alignment.max(align);
        let base = data
            .len()
            .checked_add(align - 1)
            .ok_or_else(|| error("constant alignment overflows"))?
            & !(align - 1);
        data.resize(base, 0);
        bases.insert(index.0, u32::try_from(base).map_err(error)?);
        data.extend_from_slice(section.data().map_err(error)?);
        for (offset, relocation) in section.relocations() {
            let RelocationFlags::Coff { typ } = relocation.flags() else {
                return Err(error("constant has a non-COFF relocation"));
            };
            let RelocationTarget::Symbol(symbol) = relocation.target() else {
                return Err(error("COFF relocation has no symbol index"));
            };
            let offset = u32::try_from(offset)
                .map_err(error)?
                .checked_add(bases[&index.0])
                .ok_or_else(|| error("constant relocation offset overflows"))?;
            relocations.extend_from_slice(&offset.to_le_bytes());
            relocations.extend_from_slice(&u32::try_from(symbol.0).map_err(error)?.to_le_bytes());
            relocations.extend_from_slice(&typ.to_le_bytes());
        }
    }
    output.relocate_constant_symbols(primary, &bases)?;
    output.replace_constant_sections(primary, &sections, &data, &relocations, alignment)?;
    *bytes = output.finish()?;
    Ok(())
}

impl CoffObject<'_> {
    fn relocate_constant_symbols(
        &mut self,
        primary: SectionIndex,
        bases: &BTreeMap<usize, u32>,
    ) -> Result<(), CodegenError> {
        for symbol in self.file.symbols() {
            let Some(section) = symbol.section_index() else {
                continue;
            };
            let Some(&base) = bases.get(&section.0) else {
                continue;
            };
            let offset = symbol.index().0 * self.symbol_bytes;
            let value = u32::try_from(symbol.address())
                .map_err(error)?
                .checked_add(base)
                .ok_or_else(|| error("constant symbol value overflows"))?;
            self.symbols[offset + 8..offset + 12].copy_from_slice(&value.to_le_bytes());
            if self.symbol_bytes == 18 {
                self.symbols[offset + 12..offset + 14]
                    .copy_from_slice(&i16::try_from(primary.0).map_err(error)?.to_le_bytes());
            } else {
                self.symbols[offset + 12..offset + 16]
                    .copy_from_slice(&i32::try_from(primary.0).map_err(error)?.to_le_bytes());
            }
            let class = &mut self.symbols[offset + self.symbol_bytes - 2];
            if self.section_aux.get(&section.0) == Some(&(offset + self.symbol_bytes)) {
                if section != primary {
                    // Retain the original symbol slot for relocations targeting
                    // a section base, now a local label at its merged offset.
                    // Its old auxiliary slot must not define the primary again.
                    *class = pe::IMAGE_SYM_CLASS_LABEL;
                }
            } else if *class == pe::IMAGE_SYM_CLASS_EXTERNAL {
                *class = pe::IMAGE_SYM_CLASS_STATIC;
            }
        }
        Ok(())
    }

    fn replace_constant_sections(
        &mut self,
        primary: SectionIndex,
        sections: &[SectionIndex],
        data: &[u8],
        relocations: &[u8],
        alignment: usize,
    ) -> Result<(), CodegenError> {
        let size = u32::try_from(data.len()).map_err(error)?;
        let data_offset = u32::try_from(self.bytes.len()).map_err(error)?;
        self.bytes.extend_from_slice(data);
        let relocation_offset = u32::try_from(self.bytes.len()).map_err(error)?;
        let relocation_count = u32::try_from(relocations.len() / 10).map_err(error)?;
        let overflow = relocation_count > u32::from(u16::MAX);
        if overflow {
            self.bytes.extend_from_slice(
                &relocation_count
                    .checked_add(1)
                    .ok_or_else(|| error("constant relocation count overflows"))?
                    .to_le_bytes(),
            );
            self.bytes.extend_from_slice(&[0; 6]);
        }
        self.bytes.extend_from_slice(relocations);
        let encoded_count = relocation_count.min(u32::from(u16::MAX)) as u16;
        for &index in sections {
            let header = self.sections_offset + (index.0 - 1) * 40;
            let SectionFlags::Coff { characteristics } =
                self.file.section_by_index(index).map_err(error)?.flags()
            else {
                return Err(error("constant has a non-COFF section header"));
            };
            let mut flags =
                characteristics & !(pe::IMAGE_SCN_ALIGN_MASK | pe::IMAGE_SCN_LNK_NRELOC_OVFL);
            if index == primary {
                flags |= (alignment.trailing_zeros() + 1) << 20;
                if overflow {
                    flags |= pe::IMAGE_SCN_LNK_NRELOC_OVFL;
                }
                self.bytes[header + 16..header + 20].copy_from_slice(&size.to_le_bytes());
                self.bytes[header + 20..header + 24].copy_from_slice(&data_offset.to_le_bytes());
                self.bytes[header + 24..header + 28]
                    .copy_from_slice(&relocation_offset.to_le_bytes());
                self.bytes[header + 32..header + 34].copy_from_slice(&encoded_count.to_le_bytes());
                let aux = *self
                    .section_aux
                    .get(&index.0)
                    .ok_or_else(|| error("constant section has no auxiliary record"))?;
                // A module containing only MinGW address pointers has no plain
                // .rdata yet. Keep its header and section symbol in agreement.
                self.bytes[header..header + 8].copy_from_slice(b".rdata\0\0");
                let symbol = aux - self.symbol_bytes;
                self.symbols[symbol..symbol + 8].copy_from_slice(b".rdata\0\0");
                self.symbols[aux..aux + 4].copy_from_slice(&size.to_le_bytes());
                self.symbols[aux + 4..aux + 6].copy_from_slice(&encoded_count.to_le_bytes());
                self.symbols[aux + 8..aux + 12].fill(0);
            } else {
                // Leave unused headers in place. Only the primary contribution
                // holds content; relocation symbol indices remain valid.
                flags &= !pe::IMAGE_SCN_LNK_COMDAT;
                self.bytes[header + 16..header + 36].fill(0);
            }
            self.bytes[header + 36..header + 40].copy_from_slice(&flags.to_le_bytes());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
