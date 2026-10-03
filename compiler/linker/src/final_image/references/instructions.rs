use super::*;
use ResolvedShape as Shape;

pub(super) fn check(
    image: &FinalImage<'_>,
    inputs: &ProgramInputs<'_>,
    shape: &ResolvedShape,
    encoded: u64,
    place: u64,
    pages: &mut BTreeMap<u32, u64>,
) -> Result<(), LinkError> {
    let instruction = || -> Result<u32, LinkError> {
        Ok(u32::from_le_bytes(
            image.at(place, 4)?.try_into().map_err(error)?,
        ))
    };
    match shape {
        Shape::Unsigned64 { target } => target.pointer(image, place, encoded as i64),
        Shape::Subtractor64 {
            minuend,
            subtrahend,
        } => {
            let lhs = minuend.address(image)?;
            let rhs = subtrahend.address(image)?;
            let actual = u64::from_le_bytes(image.at(place, 8)?.try_into().map_err(error)?);
            if actual != lhs.wrapping_sub(rhs).wrapping_add(encoded) {
                return Err(error("subtractor reference resolves to different owners"));
            }
            Ok(())
        }
        Shape::Branch26 { target } => {
            let instruction = instruction()?;
            if instruction & 0x7c00_0000 != 0x1400_0000 {
                return Err(error("branch relocation is not B/BL"));
            }
            let address = place
                .checked_add_signed(signed(u64::from(instruction & 0x03ff_ffff), 26) << 2)
                .ok_or_else(|| error("branch overflow"))?;
            let addend = signed(encoded & 0x03ff_ffff, 26) << 2;
            match &target {
                Expected::Symbol(name) if inputs.dynamic.contains_key(name) => {
                    stub(image, target, address, addend)
                }
                _ if target.address(image)?.checked_add_signed(addend) == Some(address) => Ok(()),
                _ => stub(image, target, address, addend).map_err(|err| error(format!(
                    "branch at {place:#x} targets {address:#x}, expected {target:?} at {:#x} + {addend}: {err}", target.address(image).unwrap_or(0)
                ))),
            }
        }
        Shape::Page21 { target, .. }
        | Shape::GotLoadPage21 { target }
        | Shape::TlvpLoadPage21 { target } => {
            let instruction = instruction()?;
            let page = adrp(instruction, place)?;
            pages.insert(instruction & 31, page);
            if let Shape::Page21 {
                explicit_addend, ..
            } = shape
            {
                let target = target.address(image)?;
                let addend = i64::from(explicit_addend.unwrap_or(0));
                if target
                    .checked_add_signed(addend)
                    .map(|address| address & !0xfff)
                    != Some(page)
                {
                    return Err(error("ADRP resolves to a different target page"));
                }
            }
            Ok(())
        }
        Shape::PageOffset12 { target, .. }
        | Shape::GotLoadPageOffset12 { target }
        | Shape::TlvpLoadPageOffset12 { target } => {
            let instruction = instruction()?;
            let page = *pages
                .get(&((instruction >> 5) & 31))
                .ok_or_else(|| error("page-offset use has no matching ADRP"))?;
            let displacement = offset(instruction)?;
            let address = page
                .checked_add(displacement)
                .ok_or_else(|| error("page-offset address overflow"))?;
            if let Shape::PageOffset12 {
                explicit_addend, ..
            } = shape
            {
                let addend =
                    i64::from(explicit_addend.unwrap_or(0)) + offset(encoded as u32)? as i64;
                if target.address(image)?.checked_add_signed(addend) != Some(address) {
                    return Err(error(format!(
                        "page-offset use does not resolve to {target:?} + {addend}"
                    )));
                }
                Ok(())
            } else if instruction & 0x1f00_0000 == 0x1100_0000 {
                if target.address(image)? != address {
                    return Err(error("relaxed GOT use resolves to another owner"));
                }
                Ok(())
            } else {
                target.pointer(image, address, 0)
            }
        }
        Shape::PointerToGot32 { target } => {
            let delta = i32::from_le_bytes(image.at(place, 4)?.try_into().map_err(error)?) as i64;
            let address = place
                .checked_add_signed(delta)
                .ok_or_else(|| error("GOT-relative address overflow"))?;
            target.pointer(image, address, 0)
        }
    }
}

fn signed(value: u64, bits: u32) -> i64 {
    ((value << (64 - bits)) as i64) >> (64 - bits)
}
fn adrp(instruction: u32, place: u64) -> Result<u64, LinkError> {
    if instruction & 0x9f00_0000 != 0x9000_0000 {
        return Err(error("page relocation is not ADRP"));
    }
    let immediate =
        ((u64::from(instruction) >> 5) & 0x7ffff) << 2 | ((u64::from(instruction) >> 29) & 3);
    (place & !0xfff)
        .checked_add_signed(signed(immediate, 21) << 12)
        .ok_or_else(|| error("ADRP address overflow"))
}
fn offset(instruction: u32) -> Result<u64, LinkError> {
    let immediate = u64::from((instruction >> 10) & 0xfff);
    if instruction & 0x1f00_0000 == 0x1100_0000 {
        return Ok(immediate << if instruction & (1 << 22) != 0 { 12 } else { 0 });
    }
    if instruction & 0x3b00_0000 == 0x3900_0000 {
        return Ok(immediate << (instruction >> 30));
    }
    Err(error(
        "page-offset relocation is not ADD or unsigned load/store",
    ))
}
fn stub(
    image: &FinalImage<'_>,
    target: &Expected,
    address: u64,
    addend: i64,
) -> Result<(), LinkError> {
    let section = image
        .file
        .section_by_name("__stubs")
        .ok_or_else(|| error("dynamic call has no stub section"))?;
    if address < section.address()
        || address
            .checked_add(12)
            .is_none_or(|end| end > section.address() + section.size())
        || (address - section.address()) % 12 != 0
    {
        return Err(error("dynamic branch does not target an exact arm64 stub"));
    }
    let bytes = image.at(address, 12)?;
    let page = adrp(
        u32::from_le_bytes(bytes[..4].try_into().map_err(error)?),
        address,
    )?;
    let load = u32::from_le_bytes(bytes[4..8].try_into().map_err(error)?);
    let branch = u32::from_le_bytes(bytes[8..12].try_into().map_err(error)?);
    if load & 0xffc0_03ff != 0xf940_0210 || branch != 0xd61f_0200 {
        return Err(error("unexpected dynamic symbol stub instructions"));
    }
    target.pointer(image, page + offset(load)?, addend)
}
