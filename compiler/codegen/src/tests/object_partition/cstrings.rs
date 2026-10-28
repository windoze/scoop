use super::*;
use object::ObjectSection;
use scoop_lir::{ObjectDefinitionAtomId, OptimizationMode};

fn message_module(odr: bool) -> (Module, ObjectDefinitionAtomId) {
    let mut module = values_module();
    let body = if odr {
        odr_callable_body("arrayOverflowMessage")
    } else {
        callable_body("arrayOverflowMessage")
    };
    let identity = scoop_lir::CallableCStringIdentity::new(
        module.cone,
        &body,
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, 0),
            [],
        ),
    )
    .unwrap();
    let atom = identity.atom_record().id();
    module.globals = Arena::new();
    let message = module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::CString {
            identity,
            value: "array size overflow".into(),
        },
    });
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        terminator_origin: Default::default(),
        name: "entry".into(),
        instructions: Vec::new().into(),
        terminator: Terminator::Return {
            value: Some(Value::Global(message)),
        },
    });
    module.functions = vec![Function {
        debug_name: Default::default(),
        debug_variables: Vec::new(),
        debug_origin: Default::default(),
        callable_body: body,
        gc_effect: GcEffect::NoGc,
        signature: plain_scoop_signature(Vec::new(), RAW_PTR),
        call_targets: CallTargets::default(),
        safepoints: scoop_lir::SafepointIdentities::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    }];
    module.output = scoop_lir::LirOutput::Library;
    (module, atom)
}

#[test]
fn callable_cstring_has_one_disjoint_definition_range() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
        scoop_target_data::DARWIN_AARCH64_ID,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            for odr in [false, true] {
                let (mut module, atom) = message_module(odr);
                module.meta = string_metadata_for(scoop_target_data::layout(target).unwrap());
                let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
                let profile = ValidatedBackendProfile::for_selection(
                    scoop_target_data::selection(target).unwrap(),
                )
                .unwrap()
                .with_optimization(mode);
                let emitted = emit_object_set(
                    &input,
                    &scoop_lir::ConeCoordinate::reserved_single_file(),
                    &[ConeIdentity::CORE],
                    scoop_lir::EntryProductionSourceV1::Library,
                    directory.path(),
                    profile,
                )
                .unwrap();
                let member = emitted
                    .members()
                    .iter()
                    .find(|member| {
                        matches!(member.units().kind(), ScoopLirObjectKindV1::CallableBody(_))
                    })
                    .unwrap();
                let bytes = std::fs::read(member.path()).unwrap();
                let file = object::File::parse(bytes.as_slice()).unwrap();
                let normalization = input
                    .module()
                    .meta
                    .target_profile
                    .contract()
                    .native_symbol_normalization();
                let ranges = member
                    .units()
                    .definition_plans()
                    .iter()
                    .flat_map(|id| {
                        emitted
                            .production()
                            .canonical_definitions()
                            .plan(*id)
                            .unwrap()
                            .atom_boundaries()
                    })
                    .map(|boundary| {
                        let start = file
                            .symbol_by_name(&normalization.compiler_generated_object_symbol(
                                boundary.start().symbol().as_str(),
                            ))
                            .unwrap();
                        let end =
                            file.symbol_by_name(&normalization.compiler_generated_object_symbol(
                                boundary.end().symbol().as_str(),
                            ))
                            .unwrap();
                        assert_eq!(start.section_index(), end.section_index());
                        (
                            boundary.atom(),
                            start.section_index().unwrap(),
                            start.address(),
                            end.address(),
                        )
                    })
                    .collect::<Vec<_>>();
                let (_, section_index, start, end) =
                    *ranges.iter().find(|range| range.0 == atom).unwrap();
                let section = file.section_by_index(section_index).unwrap();
                let offset = (start - section.address()) as usize;
                assert_eq!(end - start, 20);
                assert_eq!(
                    &section.data().unwrap()[offset..offset + 20],
                    b"array size overflow\0"
                );
                for (other, other_section, other_start, other_end) in ranges {
                    if other != atom && other_section == section_index {
                        assert!(
                            end <= other_start || other_end <= start,
                            "{target:?}/{mode:?}/odr={odr}: CString overlaps {other:?}"
                        );
                    }
                }
            }
        }
    }
}
