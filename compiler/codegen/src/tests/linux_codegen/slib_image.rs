use super::slib_support::SlibObjects;
use super::*;
use object::ObjectSymbol;
use scoop_slib::*;

#[test]
fn elf_cone_image_emits_the_runtime_record() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for module in [
            for_target(
                moving_gc::qualification::stackmap_qualification_module(),
                target,
            ),
            super::slib_metadata_fixture::module(target),
        ] {
            let fixture = SlibObjects::new(module, directory.path());
            let builtins = fixture.builtins(&fixture.objects);
            let image =
                bind_cone_image_v1(&builtins, fixture.emitted.production.image_plan().clone())
                    .unwrap();
            let bytes = &fixture
                .objects
                .iter()
                .find(|(member, _)| *member == image.member())
                .unwrap()
                .1;
            let file = object::File::parse(bytes.as_slice()).unwrap();
            let symbol = file
                .symbol_by_name(image.plan().symbol().symbol().as_str())
                .unwrap();
            let section = file
                .section_by_index(symbol.section_index().unwrap())
                .unwrap();
            let start = usize::try_from(symbol.address() - section.address()).unwrap();
            let record = &section.data().unwrap()[start..start + 208];
            assert_eq!(
                u64::from_le_bytes(record[0..8].try_into().unwrap()),
                0x5343_4f4f_5049_4d47
            );
            assert_eq!(
                u32::from_le_bytes(record[8..12].try_into().unwrap()),
                scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1
            );
            assert_eq!(u32::from_le_bytes(record[12..16].try_into().unwrap()), 208);
        }
    }
}
