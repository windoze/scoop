//! Read actual ELF stackmaps and bind their registration digest slots.

use scoop_slib::*;

use super::slib_support::{SlibObjects, candidates};
use super::*;

#[test]
fn elf_stackmaps_read_actual_roots_and_bind_digest_slots() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        let mut module = for_target(
            moving_gc::qualification::stackmap_qualification_module(),
            target,
        );
        module.output = scoop_lir::LirOutput::Library;
        let semantics = scoop_lir::StrongSafepointSemanticPlanSetV1::from_module(&module).unwrap();
        let fixture = SlibObjects::new(module, directory.path());
        let objects = &fixture.objects;
        let verify = |objects: &[(SlibMemberId, Vec<u8>)]| {
            read_scoop_stackmaps_v1(
                fixture.builtins(objects),
                semantics.clone(),
                &candidates(objects),
            )
        };
        let verified = verify(objects).expect("real ELF stackmap roots");
        let callables = fixture.callables(verified.clone());
        assert_eq!(callables.fingerprints().len(), 3);
        assert_eq!(verified.records().len(), 3);
        let mut roots = verified
            .records()
            .iter()
            .map(|record| record.normalized().canonical().root_pair_count())
            .collect::<Vec<_>>();
        roots.sort();
        assert_eq!(roots, [0, 1, 2]);

        let verify_callables = |objects: &[(SlibMemberId, Vec<u8>)]| {
            let stackmaps = verify(objects).unwrap();
            let sites = fixture.sites(stackmaps.builtins().clone());
            let production = fixture.emitted.production().registration_production();
            bind_safepoint_registrations_v1(
                stackmaps,
                sites.clone(),
                production.safepoints().clone(),
            )
            .unwrap();
            bind_callable_registrations_v1(sites, production.callables().clone())
        };
        let registrations =
            verify_callables(objects).expect("ELF callable and safepoint registrations");
        assert_eq!(registrations.registrations().len(), 3);
        for record in registrations.registrations() {
            let patch = record.body_definition_patch();
            let bytes = &objects
                .iter()
                .find(|(member, _)| *member == patch.member())
                .unwrap()
                .1;
            let start = patch.checked_offset() as usize;
            assert_eq!(&bytes[start..start + 32], &[0; 32]);
        }
    }
}
