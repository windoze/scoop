use super::*;
use object::{ObjectComdat, ObjectSection};
use scoop_lir::{CallableConstantKind, OptimizationMode};

fn numeric_switch(odr: bool, has_table: bool, with_constant: bool) -> Module {
    let mut module = constants::lookup_module();
    module.globals = Arena::new();
    let function = &mut module.functions[0];
    function.callable_body = if odr {
        odr_callable_body("numericSwitch")
    } else {
        callable_body("numericSwitch")
    };
    function.signature = plain_scoop_signature(vec![LirType::I64], LirType::I64);
    for (_, block) in function.blocks.iter_mut() {
        if block.name == "default" {
            block.terminator = Terminator::Unreachable;
        } else if let Some(index) = block.name.strip_prefix("result-") {
            let index = index.parse::<u64>().unwrap();
            block.terminator = Terminator::Return {
                value: Some(signed64(index * 31 + 7)),
            };
        }
    }
    if !has_table {
        function.blocks = Arena::new();
        function.entry = function.blocks.alloc(BasicBlock {
            terminator_origin: Default::default(),
            name: "constant".into(),
            instructions: Vec::new().into(),
            terminator: Terminator::Return {
                value: Some(signed64(7)),
            },
        });
    }
    if with_constant {
        module.globals.alloc(Global {
            address_kind: PointerKind::Raw,
            scan: RefScan::None,
            init: GlobalInit::CString {
                identity: scoop_lir::CallableCStringIdentity::new(
                    module.cone,
                    &function.callable_body,
                    scoop_identity::StructuralDefinitionPath::from_first(
                        scoop_identity::StructuralPathSegment::new(
                            scoop_identity::StructuralDefinitionSiteRole::StringConstant,
                            0,
                        ),
                        [],
                    ),
                )
                .unwrap(),
                value: "already-owned callable constant".into(),
            },
        });
    }
    module
}

#[test]
fn native_switch_tables_have_owned_ranges_and_follow_callable_odr_selection() {
    for target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID,
    ] {
        for (odr, has_table, with_constant) in [
            (false, true, false),
            (true, true, false),
            (false, true, true),
            (true, true, true),
            (false, false, true),
            (true, false, true),
        ] {
            let mut module = numeric_switch(odr, has_table, with_constant);
            module.meta = string_metadata_for(scoop_target_data::layout(target).unwrap());
            let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(OptimizationMode::Release);
            let emitted = emit_object_set(
                &input,
                &scoop_lir::ConeCoordinate::reserved_single_file(),
                &[scoop_identity::ConeIdentity::CORE],
                scoop_lir::EntryProductionSourceV1::Library,
                profile,
                std::num::NonZeroUsize::MIN,
            )
            .unwrap();
            let member = emitted
                .members
                .iter()
                .find(|member| matches!(member.units.kind(), ScoopLirObjectKindV1::CallableBody(_)))
                .unwrap();
            let plan = member
                .units
                .definition_plans()
                .iter()
                .filter_map(|id| emitted.production.canonical_definitions().plan(*id))
                .find(|plan| {
                    plan.definition_role() == scoop_lir::StrongDefinitionRole::CallableBody
                })
                .unwrap();
            let coff = input.module().meta.target_profile.native_object_format()
                == scoop_lir::NativeObjectFormat::Coff64;
            let kind = if coff {
                CallableConstantKind::Relocatable
            } else {
                CallableConstantKind::ReadOnly
            };
            let atom = crate::callable_constants::atom(plan, kind).unwrap();
            let bytes = member.bytes.clone();
            let file = object::File::parse(bytes.as_slice()).unwrap();
            let boundary = plan
                .atom_boundaries()
                .iter()
                .find(|boundary| boundary.atom() == atom);
            if !has_table {
                assert!(
                    boundary.is_none(),
                    "an existing CString is not a backend table"
                );
                continue;
            }
            let boundary = plan
                .atom_boundaries()
                .iter()
                .find(|boundary| boundary.atom() == atom)
                .unwrap_or_else(|| {
                    panic!(
                        "{target:?}/ODR={odr}: {:?}",
                        file.sections()
                            .map(|s| s.name().unwrap())
                            .collect::<Vec<_>>()
                    )
                });
            let start = file
                .symbol_by_name(boundary.start().symbol().as_str())
                .unwrap();
            let end = file
                .symbol_by_name(boundary.end().symbol().as_str())
                .unwrap();
            let table = file
                .section_by_index(start.section_index().unwrap())
                .unwrap();
            assert_eq!(
                table.name().unwrap(),
                if coff {
                    ".rdata"
                } else {
                    ".rodata.scoop.constants"
                }
            );
            assert_eq!(start.section_index(), Some(table.index()));
            assert_eq!(end.section_index(), start.section_index());
            assert_eq!(start.address(), 0);
            assert_eq!(end.address(), table.size());
            if with_constant {
                let strings = file
                    .section_by_name(if coff {
                        ".rdata$scoop.strings"
                    } else {
                        ".rodata.str1.1"
                    })
                    .unwrap();
                assert_ne!(strings.index(), table.index());
                assert_eq!(
                    strings.data().unwrap(),
                    b"already-owned callable constant\0"
                );
            }
            // LLVM can make one case the fallthrough instead of a table entry.
            let entries = table.relocations().count();
            assert!(entries >= 4);
            assert!(
                table
                    .relocations()
                    .all(|(offset, _)| offset + 4 <= end.address())
            );
            if odr {
                let group = file
                    .comdats()
                    .find(|group| {
                        file.symbol_by_index(group.symbol())
                            .unwrap()
                            .name()
                            .unwrap()
                            == plan.primary_symbol().symbol().as_str()
                    })
                    .unwrap();
                assert!(group.sections().any(|section| section == table.index()));
            }
        }
    }
}
