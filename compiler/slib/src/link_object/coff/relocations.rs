use object::pe;

use super::*;

pub(super) fn read<'data, H: CoffHeader>(
    file: &CoffFile<'data, &'data [u8], H>,
    symbols: &[ObservedObjectSymbolV1],
) -> Result<Vec<ObservedObjectRelocationV1>, CoffObjectError> {
    let mut relocations = Vec::new();
    for section in file.sections() {
        let data = section.data().map_err(error)?;
        for raw in section.coff_relocations().map_err(error)? {
            let kind = raw.typ.get(object::LittleEndian);
            if kind == pe::IMAGE_REL_AMD64_ABSOLUTE {
                continue;
            }
            let width = width(kind)?;
            let target_symbol = raw.symbol_table_index.get(object::LittleEndian);
            symbols::require_symbol(symbols, target_symbol)?;
            let offset = raw.virtual_address.get(object::LittleEndian) as usize;
            let end = offset
                .checked_add(usize::from(width))
                .ok_or_else(|| error("COFF relocation extent overflow"))?;
            let field = data
                .get(offset..end)
                .ok_or_else(|| error("COFF relocation field outside section"))?;
            let mut encoded = [0; 8];
            encoded[..usize::from(width)].copy_from_slice(field);
            relocations.push(ObservedObjectRelocationV1 {
                containing_section_ordinal: ordinal(section.index())?,
                offset: offset as u64,
                encoded_value: u64::from_le_bytes(encoded),
                shape: ObjectRelocationShapeV1::CoffAmd64 {
                    kind,
                    target_symbol,
                    width,
                },
            });
        }
    }
    Ok(relocations)
}

fn width(kind: u16) -> Result<u8, CoffObjectError> {
    match kind {
        pe::IMAGE_REL_AMD64_ADDR64 => Ok(8),
        pe::IMAGE_REL_AMD64_ADDR32
        | pe::IMAGE_REL_AMD64_ADDR32NB
        | pe::IMAGE_REL_AMD64_REL32
        | pe::IMAGE_REL_AMD64_REL32_1
        | pe::IMAGE_REL_AMD64_REL32_2
        | pe::IMAGE_REL_AMD64_REL32_3
        | pe::IMAGE_REL_AMD64_REL32_4
        | pe::IMAGE_REL_AMD64_REL32_5
        | pe::IMAGE_REL_AMD64_SECREL
        | pe::IMAGE_REL_AMD64_SSPAN32 => Ok(4),
        pe::IMAGE_REL_AMD64_SECTION => Ok(2),
        pe::IMAGE_REL_AMD64_SECREL7 => Ok(1),
        _ => Err(error(format!(
            "unsupported AMD64 relocation kind 0x{kind:04x}"
        ))),
    }
}
