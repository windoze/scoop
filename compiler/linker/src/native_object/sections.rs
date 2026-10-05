use super::*;
use scoop_slib::{
    DarwinArm64RelocationShapeV1 as Shape, DarwinArm64RelocationTargetV1 as Target,
    ObservedMachORelocationV1,
};

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
mod tests;

pub(super) fn check(
    file: &MachOFile64<'_>,
    name: &str,
    tlv: bool,
    data: &[u8],
    relocations: &[ObservedMachORelocationV1],
) -> Result<(), LinkError> {
    if name == "__eh_frame" {
        eh_frame(data)?;
    }
    if name == "__compact_unwind" {
        if data.len() % 32 != 0 {
            return Err(error("native compact unwind record is truncated"));
        }
        for record in (0..data.len()).step_by(32) {
            pointer(relocations, record as u64)?;
        }
        if relocations.iter().any(|use_| {
            !matches!(use_.offset() % 32, 0 | 16 | 24)
                || !matches!(use_.shape(), Shape::Unsigned64 { .. })
        }) {
            return Err(error("native compact unwind has an invalid pointer field"));
        }
    }
    if tlv {
        if data.len() % 24 != 0 {
            return Err(error("native TLV descriptor is truncated"));
        }
        if relocations.len() != data.len() / 24 * 2 {
            return Err(error(
                "native TLV descriptor has missing or extra relocations",
            ));
        }
        for record in (0..data.len()).step_by(24) {
            let Target::SymbolTableIndex(index) = pointer(relocations, record as u64)? else {
                return Err(error("native TLV bootstrap requires a symbol reference"));
            };
            let bootstrap = file
                .symbol_by_index(object::SymbolIndex(index as usize))
                .map_err(error)?;
            if bootstrap.name().map_err(error)? != "__tlv_bootstrap"
                || data[record + 8..record + 16] != [0; 8]
            {
                return Err(error(
                    "native TLV descriptor has invalid bootstrap or initial key",
                ));
            }
            let template = match pointer(relocations, record as u64 + 16)? {
                Target::SymbolTableIndex(index) => file
                    .symbol_by_index(object::SymbolIndex(index as usize))
                    .map_err(error)?
                    .section_index()
                    .ok_or_else(|| error("native TLV template has no section"))?,
                Target::SectionOrdinal(index) => object::SectionIndex(index.get() as usize),
            };
            let section = file.section_by_index(template).map_err(error)?;
            let kind = section.macho_section().flags.get(file.endian()) & macho::SECTION_TYPE;
            if !matches!(
                kind,
                macho::S_THREAD_LOCAL_REGULAR | macho::S_THREAD_LOCAL_ZEROFILL
            ) {
                return Err(error(
                    "native TLV descriptor does not reference a TLS template",
                ));
            }
        }
    }
    Ok(())
}

fn pointer(relocations: &[ObservedMachORelocationV1], offset: u64) -> Result<Target, LinkError> {
    let index = relocations
        .binary_search_by_key(&offset, |use_| u64::from(use_.offset()))
        .map_err(|_| {
            error(format!(
                "native section is missing pointer relocation at {offset:#x}"
            ))
        })?;
    match relocations[index].shape() {
        Shape::Unsigned64 { target } => Ok(target),
        _ => Err(error(
            "native section pointer has an invalid relocation shape",
        )),
    }
}

fn eh_frame(data: &[u8]) -> Result<(), LinkError> {
    let mut cies = BTreeSet::new();
    let mut offset = 0usize;
    while offset < data.len() {
        let start = offset;
        let length = read(data, &mut offset, 4)?;
        if length == 0 {
            if data[offset..].iter().any(|byte| *byte != 0) {
                return Err(error("native EH frame has data after its terminator"));
            }
            break;
        }
        let (length, width) = if length == u64::from(u32::MAX) {
            (read(data, &mut offset, 8)?, 8)
        } else {
            (length, 4)
        };
        let pointer = offset;
        let end = usize::try_from(length)
            .ok()
            .and_then(|length| offset.checked_add(length))
            .filter(|end| *end <= data.len())
            .ok_or_else(|| error("native EH frame record is truncated"))?;
        let cie = read(&data[..end], &mut offset, width)?;
        if cie == 0 {
            let version = read(&data[..end], &mut offset, 1)?;
            if !matches!(version, 1 | 3 | 4) || !data[offset..end].contains(&0) {
                return Err(error(
                    "native EH CIE has invalid version or augmentation string",
                ));
            }
            cies.insert(start as u64);
        } else if (pointer as u64)
            .checked_sub(cie)
            .is_none_or(|cie| !cies.contains(&cie))
        {
            return Err(error("native EH FDE refers to a missing CIE"));
        }
        offset = end;
    }
    Ok(())
}
fn read(data: &[u8], offset: &mut usize, width: usize) -> Result<u64, LinkError> {
    let end = offset
        .checked_add(width)
        .ok_or_else(|| error("native EH frame offset overflow"))?;
    let bytes = data
        .get(*offset..end)
        .ok_or_else(|| error("native EH frame record is truncated"))?;
    let mut value = [0; 8];
    value[..width].copy_from_slice(bytes);
    *offset = end;
    Ok(u64::from_le_bytes(value))
}
