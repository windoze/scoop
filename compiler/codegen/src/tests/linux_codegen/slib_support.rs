//! Shared production objects used by the ELF stackmap and metadata tests.

use super::*;
use scoop_slib::*;

mod requirements;

pub(super) struct SlibObjects {
    pub emitted: EmittedConeObjectSetV1,
    pub objects: Vec<(SlibMemberId, Vec<u8>)>,
    plans: PlannedLinkObjectMemberSetV1,
    symbols: PlannedStrongObjectSymbolSetV1,
    bridge_objects: VerifiedCBridgeProductionEnvelopeSetV1,
    patches: Vec<ProvisionalDigestPatchSiteV1>,
    profile: scoop_lir::CBridgeToolchainProfileV1,
}

impl SlibObjects {
    pub fn new(mut module: Module, directory: &Path) -> Self {
        module.output = scoop_lir::LirOutput::Library;
        let invocation =
            scoop_toolchain::resolve_linux_c_toolchain(module.meta.target_profile, None, None)
                .unwrap();
        let backend = ValidatedBackendProfile::from_selection(
            scoop_lir::ValidatedLirTargetSelection::from_id(module.meta.target_profile.id()),
        )
        .unwrap();
        let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
        let emitted = emit_object_set(
            &input,
            &scoop_lir::ConeCoordinate::reserved_single_file(),
            &[scoop_identity::ConeIdentity::CORE],
            scoop_lir::EntryProductionSourceV1::Library,
            directory,
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

        Self {
            emitted,
            objects,
            plans,
            symbols,
            bridge_objects,
            patches,
            profile: invocation.profile().clone(),
        }
    }

    pub fn builtins(
        &self,
        objects: &[(SlibMemberId, Vec<u8>)],
    ) -> VerifiedBuiltinObjectStrongRelocationSetV1 {
        verify_builtin_object_strong_relocations_v1(
            &self.plans,
            &self.symbols,
            &candidates(objects),
            self.bridge_objects.clone(),
            &[],
        )
        .expect("real ELF definitions and relocations")
    }

    pub fn sites(
        &self,
        builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
        objects: &[(SlibMemberId, Vec<u8>)],
    ) -> VerifiedScoopLirDigestPatchSiteSetV1 {
        verify_scoop_lir_digest_patch_sites_v1(
            builtins,
            self.emitted.foundation(),
            self.emitted.production().digest_finalization_plan().clone(),
            &candidates(objects),
            &self.patches,
        )
        .unwrap()
    }
}

pub(super) fn candidates(
    objects: &[(SlibMemberId, Vec<u8>)],
) -> Vec<ScoopLirObjectCandidateV1<'_>> {
    objects
        .iter()
        .map(|(member, bytes)| ScoopLirObjectCandidateV1::new(*member, bytes))
        .collect()
}

pub(super) fn set_rela_addend(
    objects: &mut [(SlibMemberId, Vec<u8>)],
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    member_id: SlibMemberId,
    atom_id: scoop_identity::ObjectDefinitionAtomId,
    offset_within_atom: u64,
    addend: i64,
) {
    use object::read::elf::SectionHeader;
    let member = builtins
        .strong_relocations()
        .members()
        .iter()
        .find(|member| member.member() == member_id)
        .unwrap();
    let atom = member
        .definitions()
        .definitions()
        .iter()
        .flat_map(|definition| definition.atoms())
        .find(|atom| atom.atom() == atom_id)
        .unwrap();
    let (_, bytes) = objects
        .iter_mut()
        .find(|(member, _)| *member == member_id)
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
                == atom.start() + offset_within_atom
            {
                field = Some(offset + 16);
            }
        }
    }
    let field = field.expect("metadata pointer RELA");
    bytes[field..field + 8].copy_from_slice(&addend.to_le_bytes());
}
