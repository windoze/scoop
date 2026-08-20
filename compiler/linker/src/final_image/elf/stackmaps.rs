use super::*;
use object::{ObjectComdat, RelocationTarget};

pub(super) fn check(image: &ElfImage<'_>, inputs: &ProgramInputs<'_>) -> Result<(), LinkError> {
    let start = image.symbol("__scoop_stackmaps_start")?;
    let end = image.symbol("__scoop_stackmaps_end")?;
    let size = end
        .checked_sub(start)
        .ok_or_else(|| error("reversed ELF stackmap bounds"))?;
    image.read_only(start, size)?;
    let actual = image.at(start, usize::try_from(size).map_err(error)?)?;
    let mut offset = 0usize;
    let mut needs_eh = false;
    let mut groups = BTreeSet::new();
    for input in &inputs.objects {
        let file: ElfFile64<'_> = ElfFile64::parse(input.bytes()).map_err(error)?;
        needs_eh |= file
            .section_by_name(".eh_frame")
            .is_some_and(|s| s.size() != 0);
        let Some(section) = file.section_by_name(".llvm_stackmaps") else {
            continue;
        };
        // The Link reader already checked ODR content equivalence. A COMDAT
        // stackmap contributes once together with its owning callable, even
        // when several Cones instantiate that same definition.
        if let Some(group) = file
            .comdats()
            .find(|group| group.sections().any(|index| index == section.index()))
            && !groups.insert(group.name().map_err(error)?.to_owned())
        {
            continue;
        }
        let expected = section.data().map_err(error)?;
        offset = offset
            .checked_add(7)
            .ok_or_else(|| error("ELF stackmap offset overflow"))?
            & !7;
        let next = offset
            .checked_add(expected.len())
            .ok_or_else(|| error("ELF stackmap length overflow"))?;
        let mut final_blob = actual
            .get(offset..next)
            .ok_or_else(|| error(format!("ELF output lost stackmaps from {}", input.origin)))?
            .to_vec();
        let mut object_blob = expected.to_vec();
        for (field, relocation) in section.relocations() {
            let RelocationTarget::Symbol(target) = relocation.target() else {
                return Err(error("ELF stackmap has a non-symbol callable"));
            };
            let symbol = file.symbol_by_index(target).map_err(error)?;
            let target = image
                .symbol(symbol.name().map_err(error)?)?
                .checked_add_signed(relocation.addend())
                .ok_or_else(|| error("ELF stackmap target overflow"))?;
            let actual_target = image.pointer(start + offset as u64 + field)?;
            if actual_target != target {
                return Err(error(format!(
                    "ELF stackmap callable {} differs for {}: {actual_target:#x} != {target:#x}",
                    symbol.name().map_err(error)?,
                    input.origin
                )));
            }
            let field = usize::try_from(field).map_err(error)?;
            object_blob
                .get_mut(field..field + 8)
                .ok_or_else(|| error("ELF stackmap relocation exceeds input"))?
                .fill(0);
            final_blob
                .get_mut(field..field + 8)
                .ok_or_else(|| error("ELF stackmap relocation exceeds output"))?
                .fill(0);
        }
        if object_blob != final_blob {
            return Err(error(format!(
                "ELF stackmap payload changed for {}",
                input.origin
            )));
        }
        offset = next;
    }
    if actual
        .get(offset..)
        .is_none_or(|padding| padding.len() > 7 || padding.iter().any(|byte| *byte != 0))
    {
        return Err(error("unexpected data after ELF stackmap blobs"));
    }
    if needs_eh
        && image
            .file
            .section_by_name(".eh_frame")
            .is_none_or(|s| s.size() == 0)
    {
        return Err(error("ELF output lost EH frames"));
    }
    Ok(())
}
