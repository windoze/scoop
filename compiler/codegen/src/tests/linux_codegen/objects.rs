//! Exercise the production object partition, aliases, and digest locations.

use object::{ObjectComdat, ObjectSymbol, SectionFlags, SymbolFlags, SymbolKind, elf};

use super::*;

#[test]
fn linux_elf_members_materialize_boundaries_tls_and_odr_groups() {
    let parent = tempfile::tempdir().unwrap();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        for odr in [false, true] {
            let mut module = for_target(exceptions_module(), target);
            module.output = scoop_lir::LirOutput::Library;
            if odr {
                module.functions[2].callable_body = odr_callable_body("elfExceptions");
                refresh_test_safepoints(&mut module.functions[2]);
            }
            let odr_body = module.functions[2].callable_body.id();
            #[cfg(target_os = "linux")]
            let odr_symbol = module.functions[2].symbol().to_string();
            let mut tls_symbols = Vec::new();
            for (name, initial) in [("elfTlsData", 17), ("elfTlsZero", 0)] {
                let identity = static_storage_identity(name);
                tls_symbols.push(identity.symbol().to_string());
                module.globals.alloc(Global {
                    address_kind: PointerKind::Raw,
                    scan: RefScan::None,
                    init: GlobalInit::RawStorage {
                        identity,
                        ty: LirType::I64,
                        thread_local: true,
                        initializer: LirConstantImage::Integer(
                            scoop_lir::LirIntegerConstant::Signed64(initial),
                        ),
                    },
                });
            }
            let profile = ValidatedBackendProfile::from_selection(
                scoop_lir::ValidatedLirTargetSelection::from_id(target),
            )
            .unwrap();
            let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
            let emitted = emit_object_set(
                &input,
                &scoop_lir::ConeCoordinate::reserved_single_file(),
                &[scoop_identity::ConeIdentity::CORE],
                scoop_lir::EntryProductionSourceV1::Library,
                parent.path(),
                profile,
            )
            .unwrap_or_else(|error| panic!("{target:?}/ODR={odr}: {error}"));
            let mut observed_tls = Vec::new();
            let mut odr_member = None;
            for member in emitted.members() {
                let bytes = std::fs::read(member.path()).unwrap();
                let envelope = scoop_slib::validate_scoop_lir_llvm_22_1_object_envelope_v1(
                    input.module().meta.target_profile,
                    &bytes,
                )
                .expect("shared slib ELF envelope");
                assert_eq!(envelope.sections().envelope().target(), target);
                scoop_slib::verify_object_stackmap_section_v3(&bytes, envelope.sections())
                    .expect("shared ELF stackmap section and function relocations");
                let file = object::File::parse(bytes.as_slice()).unwrap();
                for symbol in file
                    .symbols()
                    .filter(|symbol| symbol.kind() == SymbolKind::Tls && symbol.size() == 8)
                {
                    observed_tls.push(symbol.name().unwrap().to_string());
                }
                if let Some(stackmaps) = file.section_by_name(".llvm_stackmaps") {
                    assert!(
                        matches!(stackmaps.flags(), SectionFlags::Elf { sh_flags } if sh_flags & u64::from(elf::SHF_WRITE) != 0)
                    );
                }
                for definition in member.units().definition_plans() {
                    let plan = emitted
                        .production()
                        .canonical_definitions()
                        .plan(*definition)
                        .unwrap();
                    for boundary in plan.atom_boundaries() {
                        assert_ne!(
                            boundary.atom_role(),
                            scoop_lir::DefinitionAtomRole::CompactUnwind
                        );
                        let start_name = boundary.start().symbol();
                        let end_name = boundary.end().symbol();
                        let start = file.symbol_by_name(start_name.as_str()).unwrap();
                        let end = file.symbol_by_name(end_name.as_str()).unwrap();
                        assert_eq!(start.section_index(), end.section_index());
                        assert!(end.address() > start.address());
                        assert_eq!(end.size(), 0, "{end_name}");
                        assert!(
                            matches!(end.flags(), SymbolFlags::Elf { st_other, .. } if st_other & 3 == elf::STV_HIDDEN)
                        );
                        if plan.primary_symbol().linkage() == scoop_lir::LinkageClass::OdrWeak {
                            let group = file
                                .comdats()
                                .find(|group| {
                                    file.symbol_by_index(group.symbol())
                                        .unwrap()
                                        .name()
                                        .unwrap()
                                        == plan.primary_symbol().symbol().as_str()
                                })
                                .expect("definition COMDAT");
                            assert!(
                                group
                                    .sections()
                                    .any(|section| Some(section) == start.section_index())
                            );
                        }
                    }
                }
                for patch in member.digest_patches() {
                    let start = patch.checked_object_offset() as usize;
                    let end = start + patch.location().width_bytes() as usize;
                    assert!(bytes[start..end].iter().all(|byte| *byte == 0));
                }
                if odr
                    && matches!(member.kind(), EmittedConeObjectMemberKindV1::CallableBody { body, .. } if body == odr_body)
                {
                    odr_member = Some(member.path());
                }
            }
            observed_tls.sort();
            tls_symbols.sort();
            assert_eq!(observed_tls, tls_symbols);
            assert_eq!(odr_member.is_some(), odr);
            #[cfg(target_os = "linux")]
            if let Some(member) = odr_member {
                check_coalescing(member, parent.path(), &input, &odr_symbol, profile);
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn check_coalescing(
    member: &Path,
    parent: &Path,
    input: &scoop_lir::ConeLirOutput,
    symbol: &str,
    profile: ValidatedBackendProfile,
) {
    let output = parent.join("coalesced.o");
    let linked = std::process::Command::new("ld")
        .arg("-r")
        .arg(member)
        .arg(member)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let expected = statepoint::expectations(input.module())
        .unwrap()
        .for_function(symbol)
        .unwrap();
    let eh = artifact::eh_expectations(input.module())
        .unwrap()
        .for_function(symbol);
    profile.verify_object(&output, &expected, &eh).unwrap();
}
