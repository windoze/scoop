use super::*;

#[test]
fn real_dwarf_eh_records_reject_truncation_bad_cies_and_invalid_fde_references() {
    let fixture = crate::test_support::fixture();
    let inputs = crate::program::ProgramInputs::new(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &[],
    )
    .unwrap();
    let data = inputs
        .objects
        .iter()
        .find_map(|input| {
            let file: MachOFile64<'_> = MachOFile64::parse(input.bytes()).unwrap();
            let section = file.section_by_name("__eh_frame")?;
            (section.size() != 0).then(|| section.data().unwrap().to_vec())
        })
        .unwrap();
    eh_frame(&data).unwrap();
    let cie_size = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
    for (offset, replacement, expected) in [
        (
            0,
            (u32::MAX - 1).to_le_bytes().to_vec(),
            "EH frame record is truncated",
        ),
        (8, vec![0xff], "EH CIE has invalid version"),
        (
            cie_size + 8,
            1u32.to_le_bytes().to_vec(),
            "EH FDE refers to a missing CIE",
        ),
    ] {
        let mut bytes = data.clone();
        bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
        let message = eh_frame(&bytes).err().unwrap().to_string();
        assert!(message.contains(expected), "{message}");
    }
}
