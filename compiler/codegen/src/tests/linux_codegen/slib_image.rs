use super::slib_support::SlibObjects;
use super::*;
use scoop_slib::*;

#[test]
fn elf_cone_image_materializes_its_digest_slot() {
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
            let patches = fixture.sites(fixture.builtins(&fixture.objects));
            let image =
                bind_cone_image_v1(&patches, fixture.emitted.production().image_plan().clone())
                    .unwrap();
            let bytes = &fixture
                .objects
                .iter()
                .find(|(member, _)| *member == image.member())
                .unwrap()
                .1;
            let start = image.image_patch().checked_offset() as usize;
            assert_eq!(&bytes[start..start + 32], &[0; 32]);
        }
    }
}
