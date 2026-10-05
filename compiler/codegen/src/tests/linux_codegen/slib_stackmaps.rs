//! Feed production LLVM objects through the independent slib stackmap reader.

use scoop_slib::*;

use super::slib_support::{SlibObjects, candidates, set_rela_addend};
use super::*;

#[test]
fn elf_stackmaps_validate_actual_member_definitions_roots_and_return_pcs() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
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
            verify_scoop_lir_stackmaps_v1(
                fixture.builtins(objects),
                semantics.clone(),
                &candidates(objects),
            )
        };
        let verified = verify(objects).expect("real ELF stackmap machine and root contract");
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
            let candidates = candidates(objects);
            let sites = fixture.sites(stackmaps.builtins().clone(), objects);
            let production = fixture.emitted.production().registration_production();
            verify_strong_safepoint_registrations_v1(
                stackmaps,
                sites.clone(),
                production.safepoints().clone(),
                &candidates,
            )
            .unwrap();
            verify_strong_callable_registrations_v1(
                sites,
                production.callables().clone(),
                &candidates,
            )
        };
        let registrations =
            verify_callables(objects).expect("ELF callable and safepoint registrations");
        assert_eq!(registrations.registrations().len(), 3);
        let entry = registrations.registrations()[0].entry_relocation();
        let mut damaged_addend = objects.clone();
        set_rela_addend(
            &mut damaged_addend,
            verified.builtins(),
            entry.source_member(),
            entry.containing_atom(),
            entry.offset_within_atom(),
            8,
        );
        assert!(
            verify_callables(&damaged_addend).is_err(),
            "zero encoded field must not conceal a nonzero RELA addend"
        );

        let (index, start) = objects
            .iter()
            .enumerate()
            .find_map(|(index, (_, bytes))| {
                let file = object::File::parse(bytes.as_slice()).unwrap();
                file.section_by_name(".llvm_stackmaps")?;
                let (start, _) = file.section_by_name(".text").unwrap().file_range().unwrap();
                Some((index, start as usize))
            })
            .unwrap();
        let mut damaged = objects.clone();
        assert_eq!(damaged[index].1[start], 0x55);
        damaged[index].1[start] = 0x90;
        assert!(matches!(
            verify(&damaged),
            Err(ScoopLirStackmapValidationError::MachineCode {
                source: StackmapMachineCodeError::X86_64(
                    X86_64StackmapMachineCodeError::MissingManagedFrameChain
                ),
                ..
            })
        ));
    }
}
