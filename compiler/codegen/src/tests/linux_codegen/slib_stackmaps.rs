//! Feed production LLVM objects through the independent slib stackmap reader.

use scoop_slib::*;

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
        let invocation =
            scoop_toolchain::resolve_linux_c_toolchain(module.meta.target_profile, None, None)
                .unwrap();
        let backend = ValidatedBackendProfile::from_selection(
            scoop_lir::ValidatedLirTargetSelection::from_id(target),
        )
        .unwrap();
        let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
        let emitted = emit_object_set(
            &input,
            &scoop_lir::ConeCoordinate::reserved_single_file(),
            &[scoop_identity::ConeIdentity::CORE],
            scoop_lir::EntryProductionSourceV1::Library,
            directory.path(),
            backend,
        )
        .unwrap();
        let partition =
            scoop_lir::ProducerUnitPartitionV1::from_foundation(emitted.foundation()).unwrap();
        let plans = PlannedLinkObjectMemberSetV1::new(
            emitted.target(),
            &partition,
            emitted
                .members()
                .iter()
                .map(|member| {
                    CanonicalScoopLirObjectUnitSetV1::new(
                        member.units().definition_plans().to_vec(),
                    )
                    .unwrap()
                })
                .collect(),
            Vec::new(),
        )
        .unwrap();
        let surface =
            scoop_lir::ObjectSymbolSurfaceV1::from_foundation(emitted.foundation()).unwrap();
        let symbols =
            PlannedStrongObjectSymbolSetV1::new(emitted.target(), &surface, &plans).unwrap();
        let bridges =
            scoop_lir::GeneratedBridgePlanSetV1::from_foundation(emitted.foundation()).unwrap();
        assert!(bridges.units().is_empty());
        let production = scoop_lir::CBridgeProductionSetV1::from_generated_bridge_plan(
            &bridges,
            invocation.profile(),
        );
        let bridge_objects = verify_c_bridge_production_envelopes_v1(
            bridges,
            production,
            invocation.profile(),
            &plans,
            &[],
        )
        .unwrap();
        let mut objects = emitted
            .members()
            .iter()
            .map(|member| {
                (
                    plans
                        .member_for_definition(member.units().definition_plans()[0])
                        .unwrap(),
                    std::fs::read(member.path()).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        objects.sort_by_key(|(member, _)| *member);
        let verify = |objects: &[(SlibMemberId, Vec<u8>)]| {
            let candidates = objects
                .iter()
                .map(|(member, bytes)| ScoopLirObjectCandidateV1::new(*member, bytes))
                .collect::<Vec<_>>();
            let builtins = verify_builtin_object_strong_relocations_v1(
                &plans,
                &symbols,
                &candidates,
                bridge_objects.clone(),
                &[],
            )
            .expect("real ELF definitions and relocations");
            verify_scoop_lir_stackmaps_v1(builtins, semantics.clone(), &candidates)
        };
        let verified = verify(&objects).expect("real ELF stackmap machine and root contract");
        assert_eq!(verified.records().len(), 3);
        let mut roots = verified
            .records()
            .iter()
            .map(|record| record.normalized().canonical().root_pair_count())
            .collect::<Vec<_>>();
        roots.sort();
        assert_eq!(roots, [0, 1, 2]);

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
