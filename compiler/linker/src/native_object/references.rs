use super::*;
use scoop_slib::{
    DarwinArm64RelocationShapeV1 as Shape, DarwinArm64RelocationTargetV1 as Target,
    decode_darwin_arm64_section_relocations_v1,
};
use std::num::NonZeroU32;

pub(super) fn check(file: &MachOFile64<'_>) -> Result<(), LinkError> {
    let section_count = u32::try_from(file.sections().count()).map_err(error)?;
    let symbol_count = u32::try_from(file.macho_symbol_table().len()).map_err(error)?;
    let mut symbols = BTreeSet::new();
    for symbol in file.symbols() {
        if let Some(index) = symbol.section_index() {
            let containing = file.section_by_index(index).map_err(error)?;
            if symbol
                .address()
                .checked_sub(containing.address())
                .is_none_or(|offset| offset > containing.size())
            {
                return Err(error("native symbol is outside its section"));
            }
        }
        symbols.insert(u32::try_from(symbol.index().0).map_err(error)?);
    }
    let target = |target| -> Result<(), LinkError> {
        if let Target::SymbolTableIndex(index) = target
            && !symbols.contains(&index)
        {
            return Err(error(
                "native relocation references a debug or missing symbol",
            ));
        }
        Ok(())
    };
    for section in file.sections() {
        let ordinal = NonZeroU32::new(u32::try_from(section.index().0).map_err(error)?)
            .ok_or_else(|| error("invalid native section ordinal"))?;
        let name = section.name().map_err(error)?;
        let data = section.data().map_err(error)?;
        let raw = section.macho_relocations().map_err(error)?;
        let relocations = decode_darwin_arm64_section_relocations_v1(
            ordinal,
            data,
            object::bytes_of_slice(raw),
            section_count,
            symbol_count,
        )
        .map_err(|err| error(format!("native section {name}: {err}")))?;
        for relocation in &relocations {
            match relocation.shape() {
                Shape::Subtractor64 {
                    minuend,
                    subtrahend,
                } => {
                    target(minuend)?;
                    target(subtrahend)?;
                }
                Shape::Unsigned64 { target: value }
                | Shape::Branch26 { target: value }
                | Shape::Page21 { target: value, .. }
                | Shape::PageOffset12 { target: value, .. }
                | Shape::GotLoadPage21 { target: value }
                | Shape::GotLoadPageOffset12 { target: value }
                | Shape::PointerToGot32 { target: value }
                | Shape::TlvpLoadPage21 { target: value }
                | Shape::TlvpLoadPageOffset12 { target: value } => target(value)?,
            }
        }
        let tlv = section.macho_section().flags.get(file.endian()) & macho::SECTION_TYPE
            == macho::S_THREAD_LOCAL_VARIABLES;
        super::sections::check(file, name, tlv, data, &relocations)?;
    }
    Ok(())
}
