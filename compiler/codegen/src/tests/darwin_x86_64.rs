//! Production Mach-O emission, including the metadata consumed by the linker.

use object::{Object, ObjectSection, ObjectSymbol};

use super::*;

#[test]
fn members_preserve_dwarf_stackmaps_tls_and_definition_boundaries() {
    let target = scoop_target_data::DARWIN_X86_64_ID;
    for optimization in [
        scoop_lir::OptimizationMode::Debug,
        scoop_lir::OptimizationMode::Release,
    ] {
        for odr in [false, true] {
            let mut module = exceptions_module();
            module.meta = string_metadata_for(scoop_target_data::layout(target).unwrap());
            module.output = scoop_lir::LirOutput::Library;
            if odr {
                module.functions[2].callable_body = odr_callable_body("darwinX64Exceptions");
                refresh_test_safepoints(&mut module.functions[2]);
            }
            let identity = static_storage_identity("darwinX64Tls");
            let tls = format!("_{}", identity.symbol());
            module.globals.alloc(Global {
                address_kind: PointerKind::Raw,
                scan: RefScan::None,
                init: GlobalInit::RawStorage {
                    identity,
                    ty: LirType::I64,
                    thread_local: true,
                    initializer: LirConstantImage::Integer(
                        scoop_lir::LirIntegerConstant::Signed64(17),
                    ),
                },
            });
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(optimization);
            let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
            let emitted = emit_object_set(
                &input,
                &scoop_lir::ConeCoordinate::reserved_single_file(),
                &[scoop_identity::ConeIdentity::CORE],
                scoop_lir::EntryProductionSourceV1::Library,
                profile,
            )
            .unwrap_or_else(|error| panic!("{optimization:?}/ODR={odr}: {error}"));
            let mut saw_stackmap = false;
            let mut saw_tls = false;
            let mut saw_eh = false;
            let mut saw_lsda = false;
            for member in &emitted.members {
                let bytes = member.bytes.clone();
                let envelope = scoop_slib::validate_scoop_lir_llvm_22_1_object_envelope_v1(
                    input.module().meta.target_profile,
                    &bytes,
                )
                .unwrap();
                let file = object::File::parse(bytes.as_slice()).unwrap();
                assert_eq!(file.format(), object::BinaryFormat::MachO);
                assert_eq!(file.architecture(), object::Architecture::X86_64);
                saw_stackmap |=
                    scoop_slib::read_object_stackmap_section_v3(&bytes, envelope.sections())
                        .unwrap()
                        .is_some();
                saw_eh |= file
                    .sections()
                    .any(|section| section.size() > 0 && section.name().unwrap() == "__eh_frame");
                saw_lsda |= file.sections().any(|section| {
                    section.size() > 0 && section.name().unwrap() == "__gcc_except_tab"
                });
                if let Some(symbol) = file.symbol_by_name(&tls) {
                    saw_tls |= symbol.section_index().is_some_and(|section| {
                        file.section_by_index(section).unwrap().name().unwrap() == "__thread_vars"
                    });
                }
                for definition in member.units.definition_plans() {
                    let plan = emitted
                        .production
                        .canonical_definitions()
                        .plan(*definition)
                        .unwrap();
                    for boundary in plan.atom_boundaries() {
                        let start = file
                            .symbol_by_name(&format!("_{}", boundary.start().symbol()))
                            .unwrap();
                        let end = file
                            .symbol_by_name(&format!("_{}", boundary.end().symbol()))
                            .unwrap();
                        assert_eq!(start.section_index(), end.section_index());
                        assert!(
                            end.address() > start.address(),
                            "{:?}",
                            boundary.atom_role()
                        );
                        assert_eq!(
                            start.is_weak(),
                            boundary.start().linkage() == scoop_lir::LinkageClass::OdrWeak,
                            "{:?}",
                            boundary.atom_role()
                        );
                    }
                }
            }
            assert!(
                saw_stackmap && saw_tls && saw_eh && saw_lsda,
                "{optimization:?}/ODR={odr}: stackmap={saw_stackmap}, TLS={saw_tls}, EH={saw_eh}, LSDA={saw_lsda}"
            );
        }
    }
}
