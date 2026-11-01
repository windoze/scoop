//! Cross-emission of the production COFF members on every compiler host.

use object::{Object, ObjectComdat, ObjectSection, ObjectSymbol};

use super::*;

mod constants;
mod debug_info;
mod statepoint_pc;

#[cfg(windows)]
pub(super) fn native_command(target: TargetProfileId, triple: &str) -> std::process::Command {
    let (variable, linker_flags) = if target == scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID {
        ("SCOOP_TEST_CLANG", "-Wl,/opt:ref,/opt:noicf")
    } else if target == scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID {
        ("SCOOP_TEST_MINGW_MSVCRT", "-Wl,--gc-sections,--icf=none")
    } else {
        ("SCOOP_TEST_MINGW_UCRT", "-Wl,--gc-sections,--icf=none")
    };
    let compiler = std::env::var_os(variable)
        .unwrap_or_else(|| panic!("{variable} must select this native environment"));
    let mut command = std::process::Command::new(compiler);
    command.args(["-target", triple, "-fuse-ld=lld", linker_flags]);
    command
}

#[test]
fn windows_members_preserve_seh_stackmaps_tls_and_definition_boundaries() {
    for target in [
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID,
    ] {
        for optimization in [
            scoop_lir::OptimizationMode::Debug,
            scoop_lir::OptimizationMode::Release,
        ] {
            for odr in [false, true] {
                let mut module = exceptions_module();
                module.meta = string_metadata_for(scoop_target_data::layout(target).unwrap());
                module.output = scoop_lir::LirOutput::Library;
                if odr {
                    module.functions[2].callable_body = odr_callable_body("windowsExceptions");
                    refresh_test_safepoints(&mut module.functions[2]);
                }
                let identity = static_storage_identity("windowsTls");
                let tls = identity.symbol().to_string();
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
                .unwrap_or_else(|error| panic!("{target:?}/{optimization:?}/ODR={odr}: {error}"));
                let mut saw_stackmap = false;
                let mut saw_tls = false;
                let mut saw_seh = false;
                for member in &emitted.members {
                    let bytes = member.bytes.clone();
                    let envelope = scoop_slib::validate_scoop_lir_llvm_22_1_object_envelope_v1(
                        input.module().meta.target_profile,
                        &bytes,
                    )
                    .unwrap();
                    let file = object::File::parse(bytes.as_slice()).unwrap();
                    assert_eq!(file.format(), object::BinaryFormat::Coff);
                    let callable_sections = member
                        .units
                        .definition_plans()
                        .iter()
                        .filter_map(|id| emitted.production.canonical_definitions().plan(*id))
                        .find(|plan| {
                            plan.definition_role() == scoop_lir::StrongDefinitionRole::CallableBody
                        })
                        .map(|plan| {
                            file.comdats()
                                .find(|group| {
                                    file.symbol_by_index(group.symbol())
                                        .unwrap()
                                        .name()
                                        .unwrap()
                                        == plan.primary_symbol().symbol().as_str()
                                })
                                .unwrap()
                                .sections()
                                .collect::<Vec<_>>()
                        });
                    saw_stackmap |=
                        scoop_slib::read_object_stackmap_section_v3(&bytes, envelope.sections())
                            .unwrap()
                            .is_some();
                    saw_seh |= file.sections().any(|section| {
                        section.size() > 0 && section.name().unwrap().starts_with(".pdata")
                    });
                    if let Some(symbol) = file.symbol_by_name(&tls) {
                        saw_tls |= symbol.section_index().is_some_and(|section| {
                            file.section_by_index(section)
                                .unwrap()
                                .name()
                                .unwrap()
                                .starts_with(".tls")
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
                                .symbol_by_name(boundary.start().symbol().as_str())
                                .unwrap();
                            let end = file
                                .symbol_by_name(boundary.end().symbol().as_str())
                                .unwrap();
                            assert_eq!(start.section_index(), end.section_index());
                            if let Some(sections) = &callable_sections {
                                assert!(
                                    sections.contains(&start.section_index().unwrap()),
                                    "{:?}",
                                    plan.definition_role()
                                );
                            }
                            let observed = envelope
                                .sections()
                                .envelope()
                                .symbols()
                                .iter()
                                .find(|symbol| {
                                    symbol.name() == boundary.start().symbol().as_str().as_bytes()
                                })
                                .unwrap();
                            let expected =
                                if boundary.start().linkage() == scoop_lir::LinkageClass::OdrWeak {
                                    scoop_slib::ObjectSymbolKindV1::ExternalWeakDefinition
                                } else {
                                    scoop_slib::ObjectSymbolKindV1::ExternalStrongDefinition
                                };
                            assert_eq!(observed.kind(), expected, "{:?}", boundary.atom_role());
                            assert!(
                                end.address() > start.address(),
                                "{:?}",
                                boundary.atom_role()
                            );
                        }
                    }
                }
                assert!(
                    saw_stackmap && saw_tls && saw_seh,
                    "{target:?}/{optimization:?}/ODR={odr}: stackmap={saw_stackmap}, TLS={saw_tls}, SEH={saw_seh}"
                );
            }
        }
    }
}
