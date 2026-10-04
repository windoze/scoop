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

        let patches = emitted
            .members()
            .iter()
            .flat_map(|member| {
                let id = plans
                    .member_for_definition(member.units().definition_plans()[0])
                    .unwrap();
                member.digest_patches().iter().map(move |patch| {
                    ProvisionalDigestPatchSiteV1::new(
                        patch.location().intent(),
                        id,
                        patch.checked_object_offset(),
                        patch.location().width_bytes(),
                    )
                })
            })
            .collect::<Vec<_>>();
        let verify_callables = |objects: &[(SlibMemberId, Vec<u8>)]| {
            let stackmaps = verify(objects).unwrap();
            let candidates = objects
                .iter()
                .map(|(member, bytes)| ScoopLirObjectCandidateV1::new(*member, bytes))
                .collect::<Vec<_>>();
            let sites = verify_scoop_lir_digest_patch_sites_v1(
                stackmaps.builtins().clone(),
                emitted.foundation(),
                emitted.production().digest_finalization_plan().clone(),
                &candidates,
                &patches,
            )
            .unwrap();
            let production = emitted.production().registration_production();
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
            verify_callables(&objects).expect("ELF callable and safepoint registrations");
        assert_eq!(registrations.registrations().len(), 3);
        let entry = registrations.registrations()[0].entry_relocation();
        let mut damaged_addend = objects.clone();
        corrupt_entry_addend(&mut damaged_addend, verified.builtins(), entry);
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

fn corrupt_entry_addend(
    objects: &mut [(SlibMemberId, Vec<u8>)],
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    entry: &StrongRelocationBindingV1,
) {
    use object::read::elf::SectionHeader;
    let member = builtins
        .strong_relocations()
        .members()
        .iter()
        .find(|member| member.member() == entry.source_member())
        .unwrap();
    let atom = member
        .definitions()
        .definitions()
        .iter()
        .flat_map(|definition| definition.atoms())
        .find(|atom| atom.atom() == entry.containing_atom())
        .unwrap();
    let (_, bytes) = objects
        .iter_mut()
        .find(|(member, _)| *member == entry.source_member())
        .unwrap();
    let file = object::read::elf::ElfFile64::<object::Endianness>::parse(bytes.as_slice()).unwrap();
    let endian = file.endian();
    let mut field = None;
    for (_, section) in file.elf_section_table().enumerate() {
        if section.sh_type(endian) != object::elf::SHT_RELA
            || section.sh_info(endian) != atom.section_ordinal().get()
        {
            continue;
        }
        let start = section.sh_offset(endian) as usize;
        for offset in (start..start + section.sh_size(endian) as usize).step_by(24) {
            if u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
                == atom.start() + entry.offset_within_atom()
            {
                field = Some(offset + 16);
            }
        }
    }
    let field = field.expect("callable entry RELA");
    bytes[field..field + 8].copy_from_slice(&8i64.to_le_bytes());
}
