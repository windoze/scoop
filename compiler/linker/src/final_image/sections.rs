use super::*;
use object::RelocationTarget;

pub(super) fn check(image: &FinalImage<'_>, inputs: &ProgramInputs<'_>) -> Result<(), LinkError> {
    let mut stackmaps = Vec::new();
    let mut needs_eh = false;
    for input in &inputs.objects {
        let file: MachOFile64<'_> = MachOFile64::parse(input.bytes).map_err(error)?;
        needs_eh |= file
            .section_by_name("__eh_frame")
            .is_some_and(|section| section.size() != 0);
        if let Some(section) = file.section_by_name("__llvm_stackmaps") {
            stackmaps.push((input.origin, section.data().map_err(error)?, file));
        }
    }
    if needs_eh
        && image
            .file
            .section_by_name("__eh_frame")
            .is_none_or(|section| section.size() == 0)
    {
        return Err(error("final output lost its EH frames"));
    }
    if stackmaps.is_empty() {
        return Ok(());
    }
    let section = image
        .file
        .section_by_name("__llvm_stackmaps")
        .ok_or_else(|| error("final output lost its LLVM stackmaps"))?;
    if section.segment_name().map_err(error)? != Some("__DATA_CONST") {
        return Err(error("final stackmaps are outside __DATA_CONST"));
    }
    let actual = section.data().map_err(error)?;
    let mut offset = 0usize;
    for (origin, expected, file) in stackmaps {
        offset = offset
            .checked_add(7)
            .ok_or_else(|| error("stackmap offset overflow"))?
            & !7;
        let end = offset
            .checked_add(expected.len())
            .ok_or_else(|| error("stackmap length overflow"))?;
        let mut final_blob = actual
            .get(offset..end)
            .ok_or_else(|| error(format!("final stackmap blob is missing for {origin}")))?
            .to_vec();
        let mut object_blob = expected.to_vec();
        let functions = u32::from_le_bytes(
            expected
                .get(4..8)
                .ok_or_else(|| error("invalid input stackmap header"))?
                .try_into()
                .map_err(error)?,
        ) as usize;
        let source_section = file
            .section_by_name("__llvm_stackmaps")
            .ok_or_else(|| error("input stackmap section disappeared"))?;
        let relocations: BTreeMap<_, _> = source_section.relocations().collect();
        for index in 0..functions {
            let field = 16 + index * 24;
            let relocation = relocations
                .get(&(field as u64))
                .ok_or_else(|| error("input stackmap has no callable relocation"))?;
            let RelocationTarget::Symbol(target) = relocation.target() else {
                return Err(error("stackmap callable has no symbol target"));
            };
            let symbol = file.symbol_by_index(target).map_err(error)?;
            let target = image.symbol(symbol.name().map_err(error)?)?;
            if image.pointer(section.address() + offset as u64 + field as u64)? != target {
                return Err(error(format!(
                    "final stackmap callable binding differs for {origin}"
                )));
            }
            object_blob
                .get_mut(field..field + 8)
                .ok_or_else(|| error("input stackmap function table exceeds blob"))?
                .fill(0);
            final_blob
                .get_mut(field..field + 8)
                .ok_or_else(|| error("final stackmap function table exceeds blob"))?
                .fill(0);
        }
        if object_blob != final_blob {
            return Err(error(format!(
                "final stackmap payload changed for {origin}"
            )));
        }
        offset = end;
    }
    if actual
        .get(offset..)
        .is_none_or(|padding| padding.len() > 7 || padding.iter().any(|byte| *byte != 0))
    {
        return Err(error("unexpected data after final stackmap blobs"));
    }
    Ok(())
}
