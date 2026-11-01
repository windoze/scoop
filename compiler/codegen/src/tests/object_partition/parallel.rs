use super::*;
use std::num::NonZeroUsize;

#[test]
fn parallel_emission_preserves_complete_objects_on_every_target() {
    let source = std::sync::Arc::new(scoop_source::DebugFile {
        identity: std::sync::Arc::new(scoop_identity::SourceIdentity::single_file()),
        path: "parallel.scoop".into(),
    });
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::DARWIN_X86_64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID,
    ] {
        let mut module = exceptions_module();
        module.meta = string_metadata_for(scoop_target_data::layout(target).unwrap());
        module.output = scoop_lir::LirOutput::Library;
        module.functions[2].callable_body = odr_callable_body("parallelExceptions");
        refresh_test_safepoints(&mut module.functions[2]);
        let mut records = scoop_source::DebugRecords::default();
        for (index, function) in module.functions.iter_mut().enumerate() {
            function.debug_name = format!("parallel{index}");
            function.debug_origin = records.source(source.clone(), index as u32 + 1, 1);
            for (_, block) in function.blocks.iter_mut() {
                block.terminator_origin = function.debug_origin.clone();
            }
        }
        let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
        for optimization in [
            scoop_lir::OptimizationMode::Debug,
            scoop_lir::OptimizationMode::Release,
        ] {
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(optimization)
            .with_debug_info(scoop_lir::DebugInfoLevel::Full);
            let emit = |jobs| {
                emit_object_set(
                    &input,
                    &scoop_lir::ConeCoordinate::reserved_single_file(),
                    &[scoop_identity::ConeIdentity::CORE],
                    scoop_lir::EntryProductionSourceV1::Library,
                    profile,
                    NonZeroUsize::new(jobs).unwrap(),
                )
                .unwrap_or_else(|error| panic!("{target:?}/{optimization:?}/{jobs}: {error}"))
            };
            let serial = emit(1);
            // Reuse contexts within workers and request more workers than bodies.
            // Repeating a parallel run also changes the assignment of short bodies.
            for jobs in [2, 8, 2] {
                let parallel = emit(jobs);
                assert_eq!(serial.foundation, parallel.foundation);
                assert_eq!(serial.production, parallel.production);
                assert_eq!(serial.partition, parallel.partition);
                assert_eq!(serial.members.len(), parallel.members.len());
                for (expected, actual) in serial.members.iter().zip(&parallel.members) {
                    assert_eq!(expected.units, actual.units);
                    assert_eq!(expected.kind, actual.kind);
                    assert!(
                        expected.bytes == actual.bytes,
                        "{target:?}/{optimization:?}/{jobs}: {:?}",
                        expected.kind
                    );
                }
            }
        }
    }
}
