//! Section symbols and atom boundary symbols describe the same ELF base.

use object::read::elf::SectionHeader;
use scoop_slib::*;

use super::slib_support::{SlibObjects, candidates};
use super::*;

#[test]
fn elf_callable_fingerprints_normalize_section_bases_and_preserve_rela_addends() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        let module = for_target(
            moving_gc::qualification::stackmap_qualification_module(),
            target,
        );
        let semantics = scoop_lir::StrongSafepointSemanticPlanSetV1::from_module(&module).unwrap();
        let mut fixture = SlibObjects::new(module, directory.path());
        let fingerprints = |fixture: &SlibObjects| {
            let stackmaps = verify_scoop_lir_stackmaps_v1(
                fixture.builtins(&fixture.objects),
                semantics.clone(),
                &candidates(&fixture.objects),
            )
            .unwrap();
            fixture.callables(stackmaps).fingerprints().to_vec()
        };
        let original = fingerprints(&fixture);
        let builtins = fixture.builtins(&fixture.objects);
        let mut changed = 0;
        let mut addend_field = None;
        for (member_id, bytes) in &mut fixture.objects {
            let definitions = builtins
                .strong_relocations()
                .members()
                .iter()
                .find(|member| member.member() == *member_id)
                .unwrap()
                .definitions();
            let file = object::read::elf::ElfFile64::<object::Endianness>::parse(bytes.as_slice())
                .unwrap();
            let endian = file.endian();
            let mut replacements = Vec::new();
            for (_, section) in file.elf_section_table().enumerate() {
                if section.sh_type(endian) != object::elf::SHT_RELA {
                    continue;
                }
                let start = section.sh_offset(endian) as usize;
                for offset in (start..start + section.sh_size(endian) as usize).step_by(24) {
                    let symbol_index =
                        u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap());
                    let symbol =
                        &definitions.sections().envelope().symbols()[symbol_index as usize];
                    if symbol.kind() != ObjectSymbolKindV1::SectionBase {
                        continue;
                    }
                    let Some(boundary) = definitions.symbols().iter().find(|boundary| {
                        boundary.value() == 0
                            && Some(boundary.section_ordinal()) == symbol.section_ordinal()
                            && matches!(
                                boundary.role(),
                                PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. }
                            )
                    }) else {
                        continue;
                    };
                    replacements.push((offset + 12, boundary.table_index()));
                    if Some(section.sh_info(endian))
                        == file
                            .section_by_name(".eh_frame")
                            .map(|section| section.index().0 as u32)
                    {
                        addend_field = Some((*member_id, offset + 16));
                    }
                }
            }
            changed += replacements.len();
            for (offset, index) in replacements {
                bytes[offset..offset + 4].copy_from_slice(&index.to_le_bytes());
            }
        }
        assert!(changed > 0);
        assert_eq!(fingerprints(&fixture), original);
        // A different RELA addend must still change the owning body fingerprint.
        let (member, offset) = addend_field.unwrap();
        let bytes = &mut fixture
            .objects
            .iter_mut()
            .find(|(id, _)| *id == member)
            .unwrap()
            .1;
        let addend = i64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        bytes[offset..offset + 8].copy_from_slice(&(addend + 1).to_le_bytes());
        assert_ne!(fingerprints(&fixture), original);
    }
}
