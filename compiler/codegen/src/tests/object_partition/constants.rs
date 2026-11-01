use super::*;
use object::ObjectSection;
use scoop_lir::OptimizationMode;

pub(super) fn lookup_module() -> Module {
    let mut module = values_module();
    module.globals = Arena::new();
    let values = (0..12)
        .map(|index| {
            let text = format!("lookup-{index}");
            module.globals.alloc(Global {
                address_kind: PointerKind::Managed,
                scan: RefScan::None,
                init: GlobalInit::StringConst {
                    identity: immortal_string_identity(&text),
                    value: text,
                },
            })
        })
        .collect::<Vec<_>>();
    let mut blocks = Arena::new();
    let mut temps = Arena::new();
    let mut entry = blocks.alloc(BasicBlock {
        terminator_origin: Default::default(),
        name: "default".into(),
        instructions: Vec::new().into(),
        terminator: Terminator::Return {
            value: Some(Value::Global(values[0])),
        },
    });
    for index in (0..12).rev() {
        let result = blocks.alloc(BasicBlock {
            terminator_origin: Default::default(),
            name: format!("result-{index}"),
            instructions: Vec::new().into(),
            terminator: Terminator::Return {
                value: Some(Value::Global(values[index])),
            },
        });
        let condition = temps.alloc(Temp { ty: LirType::I1 });
        entry = blocks.alloc(BasicBlock {
            terminator_origin: Default::default(),
            name: format!("test-{index}"),
            instructions: vec![Instruction::IntegerCompare {
                out: condition,
                kind: IntegerKind::SIGNED_64,
                comparison: IntegerComparison::Equal,
                lhs: Value::Param(0),
                rhs: signed64(index as u64),
            }]
            .into(),
            terminator: Terminator::CondBr {
                cond: Value::Temp(condition),
                then_block: result,
                else_block: entry,
            },
        });
    }
    module.functions = vec![Function {
        debug_name: Default::default(),
        debug_variables: Vec::new(),
        debug_signature: Default::default(),
        debug_origin: Default::default(),
        callable_body: callable_body("lookup"),
        gc_effect: GcEffect::NoGc,
        signature: plain_scoop_signature(vec![LirType::I64], MANAGED_PTR),
        call_targets: CallTargets::default(),
        safepoints: scoop_lir::SafepointIdentities::default(),
        locals: Arena::new(),
        temps,
        blocks,
        entry,
    }];
    module.output = scoop_lir::LirOutput::Library;
    module
}

#[test]
fn machine_lookup_tables_are_owned_and_absent_tables_are_pruned() {
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let mut module = lookup_module();
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
            let atom =
                crate::callable_constants::atom(plan, scoop_lir::CallableConstantKind::Relocatable)
                    .unwrap();
            let boundary = plan
                .atom_boundaries()
                .iter()
                .find(|boundary| boundary.atom() == atom);
            let bytes = member.bytes.clone();
            let file = object::File::parse(bytes.as_slice()).unwrap();
            assert_eq!(
                boundary.is_some(),
                mode == OptimizationMode::Release && target == scoop_target_data::DARWIN_AARCH64_ID,
                "{target:?}/{mode:?}: {:?}",
                file.sections()
                    .map(|section| section.name().unwrap().to_string())
                    .collect::<Vec<_>>()
            );
            if let Some(boundary) = boundary {
                let normalization = input
                    .module()
                    .meta
                    .target_profile
                    .contract()
                    .native_symbol_normalization();
                let start = file
                    .symbol_by_name(
                        &normalization
                            .compiler_generated_object_symbol(boundary.start().symbol().as_str()),
                    )
                    .unwrap();
                let end = file
                    .symbol_by_name(
                        &normalization
                            .compiler_generated_object_symbol(boundary.end().symbol().as_str()),
                    )
                    .unwrap();
                let section = file
                    .section_by_index(start.section_index().unwrap())
                    .unwrap();
                assert_eq!(end.address() - start.address(), section.size());
                // Case zero equals the default and is folded before table creation.
                assert_eq!(section.relocations().count(), 11);
                if target == scoop_target_data::DARWIN_AARCH64_ID {
                    assert_eq!(section.segment_name().unwrap(), Some("__DATA_CONST"));
                } else {
                    assert_eq!(section.name().unwrap(), ".data.rel.ro.scoop.constants");
                }
            }
        }
    }
}
