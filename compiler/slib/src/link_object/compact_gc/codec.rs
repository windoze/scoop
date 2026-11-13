use std::collections::BTreeMap;

use super::*;

const MAGIC: u64 = 0x5343_4f4f_5047_434d;

pub(in crate::link_object) fn encode<'a>(
    frame: scoop_lir::MachineFrameRecoveryV1,
    records: impl Iterator<Item = &'a crate::VerifiedNormalizedStackmapRecordV1>,
) -> Result<Vec<u8>, CompactGcMapError> {
    let mut records = records.collect::<Vec<_>>();
    records.sort_unstable_by_key(|record| record.canonical().instruction_offset());
    let first = records
        .first()
        .ok_or_else(|| error("a GC map requires at least one site"))?;
    let first = first.canonical();
    let mut previous = None;
    let mut layouts = BTreeMap::<Vec<u32>, u32>::new();
    let mut words = Vec::<u32>::new();
    let mut sites = Vec::with_capacity(records.len());
    let (_, fp) = frame.registers();
    for record in &records {
        let record = record.canonical();
        if record.owner() != first.owner() || record.stack_size() != first.stack_size() {
            return Err(error(
                "one callable object contains different stackmap owners or frames",
            ));
        }
        if previous.is_some_and(|offset| offset >= record.instruction_offset()) {
            return Err(error("callable safepoints repeat a return PC"));
        }
        previous = Some(record.instruction_offset());
        // Normalization checked the optional Direct region table and root pairs.
        let CanonicalStackmapLocationV1::Constant {
            value_bits: deopt_count,
            ..
        } = record.locations()[2]
        else {
            unreachable!("normalized deopt count");
        };
        let mut roots = Vec::new();
        if deopt_count != 0 {
            let CanonicalStackmapLocationV1::Direct {
                dwarf_register,
                signed_offset_bits,
                ..
            } = record.locations()[3]
            else {
                unreachable!("normalized region table");
            };
            roots
                .push((signed_offset_bits as u32) | u32::from(dwarf_register == u32::from(fp)) | 2);
        }
        roots.extend(
            record.locations()[3 + deopt_count as usize..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| {
                    let CanonicalStackmapLocationV1::Indirect {
                        dwarf_register,
                        signed_offset_bits,
                        ..
                    } = pair[0]
                    else {
                        unreachable!("normalized root location");
                    };
                    (signed_offset_bits as u32) | u32::from(dwarf_register == u32::from(fp))
                }),
        );
        let layout = if let Some(offset) = layouts.get(&roots) {
            *offset
        } else {
            let offset = u32::try_from(words.len()).map_err(error)?;
            words.push(u32::try_from(roots.len()).map_err(error)?);
            words.extend_from_slice(&roots);
            layouts.insert(roots, offset);
            offset
        };
        sites.push((record.safepoint_id(), record.instruction_offset(), layout));
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&MAGIC.to_le_bytes());
    bytes.extend_from_slice(&scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&32u32.to_le_bytes());
    bytes.extend_from_slice(&first.stack_size().to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(sites.len()).map_err(error)?.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(words.len()).map_err(error)?.to_le_bytes());
    for (id, pc, layout) in sites {
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(&pc.to_le_bytes());
        bytes.extend_from_slice(&layout.to_le_bytes());
    }
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    append(&mut bytes, &[], 8)?;
    Ok(bytes)
}
