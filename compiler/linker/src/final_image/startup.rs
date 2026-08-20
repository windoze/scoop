use super::*;
use object::{RelocationFlags, RelocationTarget};

pub(super) fn check(
    image: &FinalImage<'_>,
    startup: &StartupObject,
    inputs: &ProgramInputs<'_>,
) -> Result<(), LinkError> {
    let file: MachOFile64<'_> = MachOFile64::parse(startup.bytes.as_slice()).map_err(error)?;
    let text = file
        .section_by_name("__text")
        .ok_or_else(|| error("startup has no text"))?;
    let main = file
        .symbols()
        .find(|symbol| symbol.name().ok() == Some("_main"))
        .ok_or_else(|| error("startup has no main"))?;
    let array = file
        .symbols()
        .find(|symbol| symbol.name().ok() == Some(crate::startup::IMAGE_ARRAY))
        .ok_or_else(|| error("startup has no image array symbol"))?;
    let final_main = image.symbol("_main")?;
    let mut pages = BTreeMap::new();
    let mut root_references = 0;
    let mut entry_calls = 0;
    let mut relocations: Vec<_> = text.relocations().collect();
    relocations.sort_by_key(|(offset, _)| *offset);
    for (offset, relocation) in relocations {
        let RelocationTarget::Symbol(target) = relocation.target() else {
            return Err(error("startup text relocation has no symbol target"));
        };
        let symbol = file.symbol_by_index(target).map_err(error)?;
        let name = symbol.name().map_err(error)?;
        let name = if symbol.is_definition()
            && symbol.address() == array.address()
            && symbol.section_index() == array.section_index()
        {
            crate::startup::IMAGE_ARRAY
        } else {
            name
        };
        let expected = image.symbol(name)?;
        let place = final_main + text.address() + offset - main.address();
        let instruction = u32::from_le_bytes(image.at(place, 4)?.try_into().map_err(error)?);
        let RelocationFlags::MachO { r_type, .. } = relocation.flags() else {
            return Err(error("startup relocation is not Mach-O"));
        };
        match r_type {
            macho::ARM64_RELOC_BRANCH26 => {
                let displacement = sign_extend(u64::from(instruction & 0x03ff_ffff), 26) << 2;
                if place.checked_add_signed(displacement) != Some(expected) {
                    return Err(error(format!("startup branch does not bind to {name}")));
                }
                if name == "_scoop_rt_run_program" {
                    entry_calls += 1;
                }
            }
            macho::ARM64_RELOC_PAGE21 | macho::ARM64_RELOC_GOT_LOAD_PAGE21 => {
                if instruction & 0x9f00_0000 != 0x9000_0000 {
                    return Err(error("startup page relocation is not ADRP"));
                }
                let immediate = ((u64::from(instruction) >> 5) & 0x7ffff) << 2
                    | ((u64::from(instruction) >> 29) & 3);
                let page = (place & !0xfff)
                    .checked_add_signed(sign_extend(immediate, 21) << 12)
                    .ok_or_else(|| error("startup page address overflow"))?;
                pages.insert(instruction & 31, page);
            }
            macho::ARM64_RELOC_PAGEOFF12 | macho::ARM64_RELOC_GOT_LOAD_PAGEOFF12 => {
                let page = *pages
                    .get(&((instruction >> 5) & 31))
                    .ok_or_else(|| error("startup page-offset has no ADRP base"))?;
                let immediate = u64::from((instruction >> 10) & 0xfff);
                let actual = if instruction & 0x1f00_0000 == 0x1100_0000 {
                    page + (immediate << if instruction & (1 << 22) != 0 { 12 } else { 0 })
                } else if instruction & 0x3b00_0000 == 0x3900_0000 {
                    image.pointer(page + (immediate << (instruction >> 30)))?
                } else {
                    return Err(error("startup page-offset has an unexpected instruction"));
                };
                if actual != expected {
                    return Err(error(format!("startup reference does not bind to {name}")));
                }
                if name == inputs.root {
                    root_references += 1;
                }
            }
            _ => {
                return Err(error(format!(
                    "unexpected startup relocation kind {r_type}"
                )));
            }
        }
    }
    if root_references != 1 || entry_calls != 1 {
        return Err(error(
            "startup must reference its root and call run_program exactly once",
        ));
    }
    Ok(())
}

fn sign_extend(value: u64, bits: u32) -> i64 {
    ((value << (64 - bits)) as i64) >> (64 - bits)
}
