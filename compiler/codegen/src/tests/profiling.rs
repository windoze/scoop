use super::*;
use object::{Object, ObjectSection};
use scoop_profile::{BodyId, BodyKey, BodyLayout, Counter, Location, Position, PositionKind};

#[test]
fn profiling_counters_survive_production_object_emission() {
    for (target, odr) in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID,
    ]
    .into_iter()
    .flat_map(|target| [false, true].map(|odr| (target, odr)))
    {
        let mut module = values_module();
        module.output = scoop_lir::LirOutput::Library;
        module.meta = string_metadata_for(scoop_target_data::layout(target).unwrap());
        if odr {
            module.functions[0].callable_body = odr_callable_body("profiled-shared");
            refresh_test_safepoints(&mut module.functions[0]);
        }
        for function in &mut module.functions {
            let key = BodyKey {
                body: BodyId::from_key(function.callable_body.identity_record().key()),
                fingerprint: scoop_wire::Digest256::from_array([7; 32]).into(),
            };
            module.meta.profile_bodies.push(BodyLayout {
                key,
                name: function.debug_name.clone(),
                positions: vec![Position {
                    kind: PositionKind::Entry,
                }],
            });
            function.blocks[function.entry].instructions.insert(
                0,
                Instruction::ProfileCounter(Counter {
                    location: Location {
                        body: key,
                        position: 0,
                    },
                    inlined_at: Vec::new(),
                }),
            );
        }
        let map = crate::profiling::profile_map(&module, "test-target");
        assert_eq!(map.records.len(), module.functions.len());
        let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
        let profile =
            ValidatedBackendProfile::for_selection(scoop_target_data::selection(target).unwrap())
                .unwrap()
                .with_optimization(scoop_lir::OptimizationMode::Release);
        let emitted = emit_object_set(
            &input,
            &scoop_lir::ConeCoordinate::reserved_single_file(),
            &[scoop_identity::ConeIdentity::CORE],
            scoop_lir::EntryProductionSourceV1::Library,
            profile,
            std::num::NonZeroUsize::MIN,
        )
        .unwrap_or_else(|error| panic!("{target:?}/ODR={odr}: {error}"));
        let partition =
            scoop_lir::ProducerUnitPartitionV1::from_foundation(&emitted.foundation).unwrap();
        let plans = scoop_slib::PlannedLinkObjectMemberSetV1::new(
            emitted.target(),
            &partition,
            emitted
                .members
                .iter()
                .map(|member| {
                    scoop_slib::CanonicalScoopLirObjectUnitSetV1::new(
                        member.units.definition_plans().to_vec(),
                    )
                    .unwrap()
                })
                .collect(),
            Vec::new(),
        )
        .unwrap();
        let symbols = scoop_slib::PlannedStrongObjectSymbolSetV1::new(
            emitted.target(),
            emitted.production.canonical_definitions(),
            &plans,
        )
        .unwrap();
        let mut instrumented = 0;
        for member in &emitted.members {
            let sections = scoop_slib::validate_scoop_lir_llvm_22_1_object_envelope_v1(
                emitted.target(),
                &member.bytes,
            )
            .unwrap()
            .into_sections();
            let id = plans
                .member_for_definition(member.units.definition_plans()[0])
                .unwrap();
            let definitions = scoop_slib::read_member_object_definitions_v1(
                &member.bytes,
                sections,
                symbols.member(id).unwrap(),
            )
            .unwrap();
            scoop_slib::verify_member_object_relocations_v1(definitions)
                .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
            let file = object::File::parse(member.bytes.as_slice()).unwrap();
            let has_counters = file.sections().any(|section| {
                let name = section.name().unwrap_or_default();
                name.contains("prf_cnts") || name.starts_with(".lprfc")
            });
            if matches!(member.units.kind(), ScoopLirObjectKindV1::CallableBody(_)) {
                assert!(has_counters, "{target:?}: callable lost profile counters");
                instrumented += 1;
            } else {
                assert!(!has_counters, "metadata does not own callable counts");
            }
        }
        assert_eq!(instrumented, map.records.len());
    }
}
