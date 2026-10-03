use super::*;
use object::macho;
use object::read::macho::{FatArch, MachOFatFile32, MachOFatFile64};

pub(super) fn select(bytes: &[u8]) -> Result<Range<usize>, LinkError> {
    match object::FileKind::parse(bytes).map_err(error)? {
        object::FileKind::MachOFat32 => {
            select_fat(bytes, MachOFatFile32::parse(bytes).map_err(error)?.arches())
        }
        object::FileKind::MachOFat64 => {
            select_fat(bytes, MachOFatFile64::parse(bytes).map_err(error)?.arches())
        }
        _ => Ok(0..bytes.len()),
    }
}

fn select_fat<F: FatArch>(bytes: &[u8], arches: &[F]) -> Result<Range<usize>, LinkError> {
    let header_end = 8usize
        .checked_add(std::mem::size_of_val(arches))
        .ok_or_else(|| error("universal slice table overflow"))?;
    let mut ranges = Vec::new();
    let mut selected = None;
    for arch in arches {
        let (offset, length) = arch.file_range();
        let end = offset
            .checked_add(length)
            .ok_or_else(|| error("universal slice range overflow"))?;
        let alignment = 1u64
            .checked_shl(arch.align())
            .ok_or_else(|| error("universal slice alignment overflow"))?;
        if offset < header_end as u64
            || end > bytes.len() as u64
            || length == 0
            || offset % alignment != 0
        {
            return Err(error(
                "universal slice exceeds file, overlaps its table or has invalid alignment",
            ));
        }
        ranges.push((offset, end));
        if arch.cputype() == macho::CPU_TYPE_ARM64
            && arch.cpusubtype() == macho::CPU_SUBTYPE_ARM64_ALL
            && selected.replace(offset as usize..end as usize).is_some()
        {
            return Err(error("universal input has duplicate arm64 slices"));
        }
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(error("universal input has overlapping slices"));
    }
    let selected =
        selected.ok_or_else(|| error("universal input has no compatible arm64 slice"))?;
    if matches!(
        object::FileKind::parse(&bytes[selected.clone()]).map_err(error)?,
        object::FileKind::MachOFat32 | object::FileKind::MachOFat64
    ) {
        return Err(error("nested universal native input"));
    }
    Ok(selected)
}
