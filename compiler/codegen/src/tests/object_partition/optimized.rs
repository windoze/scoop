use super::*;
use object::ObjectSection;
use scoop_lir::{DefinitionAtomRole, OptimizationMode, TargetProfileId};

fn profile(target: TargetProfileId, mode: OptimizationMode) -> ValidatedBackendProfile {
    ValidatedBackendProfile::from_selection(scoop_lir::ValidatedLirTargetSelection::from_id(target))
        .unwrap()
        .with_optimization(mode)
}

fn input_for(mut module: Module, target: TargetProfileId) -> scoop_lir::ConeLirOutput {
    module.output = scoop_lir::LirOutput::Library;
    module.meta = string_metadata_for(scoop_lir::LirTargetProfile::from_id(target));
    scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap()
}

fn pair_roots(null: bool) -> Module {
    let mut module = moving_gc::qualification::stackmap_qualification_module();
    let mut function = module.functions.remove(1);
    let pair = LirType::Aggregate(vec![MANAGED_PTR, MANAGED_PTR]);
    function.signature = plain_scoop_signature(vec![MANAGED_PTR], pair.clone());
    let result = function.temps.alloc(Temp { ty: pair.clone() });
    let block = &mut function.blocks[function.entry];
    let Instruction::ManagedPoll { site } = &mut block.instructions[0] else {
        panic!("poll fixture");
    };
    site.live = statepoint_live(vec![statepoint_value(
        scoop_lir::CallerRootSource::Temp(result),
        pair,
        &[0, 8],
    )]);
    let value = if null {
        Value::NullPointer(PointerKind::Managed)
    } else {
        Value::Param(0)
    };
    block.instructions.insert(
        0,
        Instruction::MakeAggregate {
            out: result,
            elements: vec![value, value],
        },
    );
    block.terminator = Terminator::Return {
        value: Some(Value::Temp(result)),
    };
    module.functions = vec![function];
    module
}

#[test]
fn emitted_registration_and_stackmap_use_final_ssa_root_count() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        TargetProfileId::DarwinAarch64,
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        for null in [false, true] {
            let input = input_for(pair_roots(null), target);
            let emitted = emit_object_set(
                &input,
                &scoop_lir::ConeCoordinate::reserved_single_file(),
                &[scoop_identity::ConeIdentity::CORE],
                scoop_lir::EntryProductionSourceV1::Library,
                directory.path(),
                profile(target, OptimizationMode::Release),
            )
            .unwrap_or_else(|error| panic!("{target:?}/null={null}: {error}"));
            let registrations = emitted
                .production()
                .registration_production()
                .safepoints()
                .registrations();
            assert_eq!(registrations.len(), 1);
            // Conditional polls preserve separate root leaves, including null slots.
            let expected = 2;
            assert_eq!(registrations[0].root_pair_count(), expected);
            let normalization = input
                .module()
                .meta
                .target_profile
                .contract()
                .native_symbol_normalization();
            let symbol = normalization
                .compiler_generated_object_symbol(registrations[0].symbol().symbol().as_str());
            let bytes = std::fs::read(
                emitted
                    .members()
                    .iter()
                    .find(|member| {
                        matches!(member.units().kind(), ScoopLirObjectKindV1::CallableBody(_))
                    })
                    .unwrap()
                    .path(),
            )
            .unwrap();
            let file = object::File::parse(bytes.as_slice()).unwrap();
            let record = file.symbol_by_name(&symbol).unwrap();
            let section = file
                .section_by_index(record.section_index().unwrap())
                .unwrap();
            let offset = (record.address() - section.address()) as usize;
            let data = section.data().unwrap();
            // ABI 5: prefix + identity + site id + role precede root_pair_count.
            assert_eq!(
                u32::from_le_bytes(data[offset + 132..offset + 136].try_into().unwrap()),
                expected
            );
        }
    }
}

fn dead_invokes_module() -> Module {
    let mut module = exceptions_module();
    let function = &mut module.functions[2];
    function.callable_body = odr_callable_body("optimizedDeadInvokes");
    function.signature = plain_scoop_signature(vec![METADATA_PTR, LirType::I64], LirType::I64);
    let first = function.temps.alloc(Temp { ty: LirType::I64 });
    let second = function.temps.alloc(Temp { ty: LirType::I64 });
    let condition = function.temps.alloc(Temp { ty: LirType::I1 });
    let done = function.blocks.alloc(BasicBlock {
        name: "skip_invokes".into(),
        instructions: Vec::new(),
        terminator: Terminator::Return {
            value: Some(signed64(42)),
        },
    });
    let entry = function.blocks.alloc(BasicBlock {
        name: "equal_expressions".into(),
        instructions: vec![
            Instruction::IntegerBinary {
                out: first,
                kind: IntegerKind::SIGNED_64,
                operation: IntegerBinaryOperation::Add,
                lhs: Value::Param(1),
                rhs: signed64(3),
            },
            Instruction::IntegerBinary {
                out: second,
                kind: IntegerKind::SIGNED_64,
                operation: IntegerBinaryOperation::Add,
                lhs: Value::Param(1),
                rhs: signed64(3),
            },
            Instruction::IntegerCompare {
                out: condition,
                kind: IntegerKind::SIGNED_64,
                comparison: IntegerComparison::Equal,
                lhs: Value::Temp(first),
                rhs: Value::Temp(second),
            },
        ],
        terminator: Terminator::CondBr {
            cond: Value::Temp(condition),
            then_block: done,
            else_block: function.entry,
        },
    });
    function.entry = entry;
    refresh_test_safepoints(function);
    module
}

#[test]
fn deleted_invokes_leave_no_site_registration_image_entry_or_backend_atom() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        TargetProfileId::DarwinAarch64,
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let input = input_for(dead_invokes_module(), target);
            let body = input.module().functions[2].callable_body.id();
            let emitted = emit_object_set(
                &input,
                &scoop_lir::ConeCoordinate::reserved_single_file(),
                &[scoop_identity::ConeIdentity::CORE],
                scoop_lir::EntryProductionSourceV1::Library,
                directory.path(),
                profile(target, mode),
            )
            .unwrap_or_else(|error| panic!("{target:?}/{mode:?}: {error}"));
            let expected_sites = if mode == OptimizationMode::Release {
                0
            } else {
                3
            };
            assert_eq!(
                emitted
                    .production()
                    .registration_production()
                    .safepoints()
                    .registrations()
                    .len(),
                expected_sites
            );
            assert_eq!(
                emitted
                    .production()
                    .image_plan()
                    .tables()
                    .safepoints()
                    .len(),
                expected_sites
            );
            let projected = ScoopLirObjectPartitionV1::from_foundation(
                &input,
                emitted.foundation(),
                emitted.production().canonical_definitions(),
            )
            .unwrap();
            assert_eq!(&projected, emitted.partition());
            let member = emitted
                .members()
                .iter()
                .find(|member| member.units().kind() == ScoopLirObjectKindV1::CallableBody(body))
                .unwrap();
            let plan = member
                .units()
                .definition_plans()
                .iter()
                .filter_map(|id| emitted.production().canonical_definitions().plan(*id))
                .find(|plan| {
                    plan.definition_role() == scoop_lir::StrongDefinitionRole::CallableBody
                })
                .unwrap();
            let roles = plan
                .atom_boundaries()
                .iter()
                .map(|atom| atom.atom_role())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                roles.contains(&DefinitionAtomRole::Stackmap),
                expected_sites != 0
            );
            assert_eq!(
                roles.contains(&DefinitionAtomRole::Lsda),
                expected_sites != 0
            );
            if expected_sites == 0 {
                assert_eq!(
                    member.units().definition_plans().len(),
                    2,
                    "body and callable registration survive"
                );
                assert_eq!(
                    emitted.foundation().definition_plan_count() + 3,
                    input.foundation().definition_plan_count()
                );
            }
        }
    }
}
