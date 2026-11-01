//! Shared production objects used by the ELF stackmap and metadata tests.

use super::*;
use scoop_slib::*;

mod requirements;

pub(super) struct SlibObjects {
    pub emitted: EmittedConeObjectSetV1,
    pub objects: Vec<(SlibMemberId, Vec<u8>)>,
    plans: PlannedLinkObjectMemberSetV1,
    symbols: PlannedStrongObjectSymbolSetV1,
    bridge_objects: GeneratedCObjectSetV1,
    c_objects: Vec<(SlibMemberId, Vec<u8>)>,
}

impl SlibObjects {
    pub fn new(mut module: Module, directory: &Path) -> Self {
        module.output = scoop_lir::LirOutput::Library;
        let invocation =
            scoop_toolchain::resolve_linux_c_toolchain(module.meta.target_profile, None, None)
                .unwrap();
        let backend = ValidatedBackendProfile::for_selection(
            scoop_target_data::selection(module.meta.target_profile.id())
                .expect("implemented target selected by this consumer"),
        )
        .unwrap();
        let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
        let emitted = emit_object_set(
            &input,
            &scoop_lir::ConeCoordinate::reserved_single_file(),
            &[scoop_identity::ConeIdentity::CORE],
            scoop_lir::EntryProductionSourceV1::Library,
            backend,
        )
        .unwrap();
        let c_emitted =
            emit_c_bridge_object_set(&input, directory, invocation.profile(), |source, object| {
                invocation
                    .compile_object(source, object)
                    .map_err(|error| CodegenError(error.to_string()))
            })
            .unwrap();
        let partition =
            scoop_lir::ProducerUnitPartitionV1::from_foundation(&emitted.foundation).unwrap();
        let plans = PlannedLinkObjectMemberSetV1::new(
            emitted.target(),
            &partition,
            emitted
                .members
                .iter()
                .map(|member| {
                    CanonicalScoopLirObjectUnitSetV1::new(member.units.definition_plans().to_vec())
                        .unwrap()
                })
                .collect(),
            c_emitted
                .members()
                .iter()
                .map(|member| {
                    CanonicalGeneratedBridgeObjectUnitSetV1::new(vec![member.unit()]).unwrap()
                })
                .collect(),
        )
        .unwrap();
        let surface =
            scoop_lir::ObjectSymbolSurfaceV1::from_foundation(&emitted.foundation).unwrap();
        let symbols =
            PlannedStrongObjectSymbolSetV1::new(emitted.target(), &surface, &plans).unwrap();
        let bridges =
            scoop_lir::GeneratedBridgePlanSetV1::from_foundation(&emitted.foundation).unwrap();
        let mut c_objects = c_emitted
            .members()
            .iter()
            .map(|member| {
                (
                    plans
                        .member_for_generated_bridge_unit(member.unit())
                        .unwrap(),
                    std::fs::read(member.object_path()).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        c_objects.sort_by_key(|(member, _)| *member);
        let production = scoop_lir::CBridgeProductionSetV1::from_generated_bridge_plan(
            &bridges,
            invocation.profile(),
        );
        let bridge_objects = read_generated_c_objects_v1(
            production,
            invocation.profile(),
            &c_candidates(&c_objects),
        )
        .unwrap();
        let mut objects = emitted
            .members
            .iter()
            .map(|member| {
                (
                    plans
                        .member_for_definition(member.units.definition_plans()[0])
                        .unwrap(),
                    member.bytes.clone(),
                )
            })
            .collect::<Vec<_>>();
        objects.sort_by_key(|(member, _)| *member);
        Self {
            emitted,
            objects,
            plans,
            symbols,
            bridge_objects,
            c_objects,
        }
    }

    pub fn builtins(
        &self,
        objects: &[(SlibMemberId, Vec<u8>)],
    ) -> VerifiedBuiltinObjectStrongRelocationSetV1 {
        self.try_builtins(objects)
            .expect("real ELF definitions and relocations")
    }

    pub fn try_builtins(
        &self,
        objects: &[(SlibMemberId, Vec<u8>)],
    ) -> Result<VerifiedBuiltinObjectStrongRelocationSetV1, BuiltinObjectSetValidationError> {
        verify_builtin_object_strong_relocations_v1(
            &self.plans,
            &self.symbols,
            &candidates(objects),
            self.bridge_objects.clone(),
            &c_candidates(&self.c_objects),
        )
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

fn c_candidates(objects: &[(SlibMemberId, Vec<u8>)]) -> Vec<GeneratedCBridgeObjectCandidateV1<'_>> {
    objects
        .iter()
        .map(|(member, bytes)| GeneratedCBridgeObjectCandidateV1::new(*member, bytes))
        .collect()
}
