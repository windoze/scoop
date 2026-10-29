use super::*;

#[cfg(all(test, windows))]
mod tests;

impl CoffObject<'_> {
    pub(super) fn order_associated_sections(&mut self) -> Result<(), CodegenError> {
        let count = self.file.sections().count();
        let mut parents = vec![None; count];
        let mut forward = false;
        for (&section, &aux) in &self.section_aux {
            let header = self.sections_offset + (section - 1) * 40;
            if field(&self.bytes, header + 36)? & pe::IMAGE_SCN_LNK_COMDAT == 0
                || self.symbols[aux + 14] != pe::IMAGE_COMDAT_SELECT_ASSOCIATIVE
            {
                continue;
            }
            let low = u16::from_le_bytes(self.symbols[aux + 12..aux + 14].try_into().unwrap());
            let high = if self.symbol_bytes == 20 {
                u16::from_le_bytes(self.symbols[aux + 16..aux + 18].try_into().unwrap())
            } else {
                0
            };
            let parent = usize::from(low) | (usize::from(high) << 16);
            if parent == 0 || parent > count || parent == section {
                return Err(error("invalid associative COMDAT section"));
            }
            parents[section - 1] = Some(parent - 1);
            forward |= parent > section;
        }
        if !forward {
            return Ok(());
        }

        let mut children = vec![Vec::new(); count];
        let mut ready = BTreeSet::new();
        for (section, parent) in parents.iter().enumerate() {
            if let Some(parent) = *parent {
                children[parent].push(section);
            } else {
                ready.insert(section);
            }
        }
        let mut order = Vec::with_capacity(count);
        while let Some(section) = ready.pop_first() {
            order.push(section);
            ready.extend(children[section].iter().copied());
        }
        if order.len() != count {
            return Err(error("cyclic associative COMDAT sections"));
        }
        let mut numbers = vec![0u32; count];
        for (index, &old) in order.iter().enumerate() {
            numbers[old] = u32::try_from(index + 1).map_err(error)?;
        }
        let end = self.sections_offset + count * 40;
        let headers = self.bytes[self.sections_offset..end].to_vec();
        for (index, old) in order.into_iter().enumerate() {
            let header = self.sections_offset + index * 40;
            self.bytes[header..header + 40].copy_from_slice(&headers[old * 40..old * 40 + 40]);
        }

        // Relocations and function/weak auxiliary records refer to symbol
        // indices, which stay fixed. Only positive section numbers move.
        let mut offset = 0;
        while offset < self.symbols.len() {
            let symbol = &mut self.symbols[offset..offset + self.symbol_bytes];
            let section = if self.symbol_bytes == 18 {
                i32::from(i16::from_le_bytes(symbol[12..14].try_into().unwrap()))
            } else {
                i32::from_le_bytes(symbol[12..16].try_into().unwrap())
            };
            if section > 0 {
                let number = *numbers
                    .get(section as usize - 1)
                    .ok_or_else(|| error("symbol references an invalid section"))?;
                if self.symbol_bytes == 18 {
                    symbol[12..14]
                        .copy_from_slice(&i16::try_from(number).map_err(error)?.to_le_bytes());
                } else {
                    symbol[12..16]
                        .copy_from_slice(&i32::try_from(number).map_err(error)?.to_le_bytes());
                }
            }
            offset += self.symbol_bytes * (1 + usize::from(symbol[self.symbol_bytes - 1]));
        }
        for (&section, &aux) in &self.section_aux {
            if let Some(parent) = parents[section - 1] {
                let number = numbers[parent];
                self.symbols[aux + 12..aux + 14].copy_from_slice(&(number as u16).to_le_bytes());
                self.symbols[aux + 16..aux + 18]
                    .copy_from_slice(&((number >> 16) as u16).to_le_bytes());
            }
        }
        Ok(())
    }
}
