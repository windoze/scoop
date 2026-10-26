use super::*;

#[test]
fn fork_restores_its_snapshot_before_initializing_the_new_task() {
    for mode in [lir::OptimizationMode::Debug, lir::OptimizationMode::Release] {
        let task =
            mir::ContextStorageType::new(ConeIdentity::SINGLE_FILE, mir::ContextStorageRole::Task);
        let node =
            mir::ContextStorageType::new(ConeIdentity::SINGLE_FILE, mir::ContextStorageRole::Node);
        let mut builder = Builder::new();
        let mut locals = Arena::new();
        let root = locals.alloc(local("snapshot", mir::Type::Context(node)));
        let fork = builder.user_fn_body(
            "fork",
            vec![param("snapshot", mir::Type::Context(node), root)],
            mir::Type::Context(task),
            returning_body(
                locals,
                expr(
                    mir::Type::Context(task),
                    mir::ExprKind::Context(mir::ContextOperation::Fork {
                        root: Box::new(local_expr(root, mir::Type::Context(node))),
                    }),
                ),
            ),
        );
        let mut module = builder.finish(fork);
        retain_context_metadata(&mut module, [task, node]);
        let output = lower_test_input_with_optimization(&seal_strong_input(module), mode).unwrap();
        let module = output.module();
        let function = &module.functions[0];
        let instructions = instructions_without_polls(&function.blocks[function.entry]);
        let lir::Instruction::Call {
            site: lir::CallSite::Managed(site),
        } = instructions[0]
        else {
            panic!("fork starts with an ordinary managed allocation");
        };
        let call = function.call_targets.typed_call_view(
            &site.call,
            &function.call_targets.managed_targets,
            lir::ManagedCallDestination::view,
        );
        assert_eq!(
            call.destination(),
            lir::CallDestination::Runtime(lir::RuntimeFunction::Managed(
                lir::ManagedRuntimeFunction::Alloc
            ))
        );
        assert_eq!(
            call.args()[1].logical_value(),
            lir::Value::MachineScalar(lir::MachineScalarValue::ByteSize(24))
        );
        let lir::Value::TypeDescriptor(td) = call.args()[0].logical_value() else {
            panic!("allocation uses the actual TaskContext descriptor");
        };
        let descriptor = descriptor(module, td);
        assert_eq!(descriptor.instance_shape.minimum_size(), 24);
        assert_eq!(fixed_scan(descriptor), &lir::RefScan::References(vec![16]));

        let roots = site.live.as_slice();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].source, lir::CallerRootSource::Param(0));
        assert!(roots[0].restore);
        assert_eq!(roots[0].leaves.as_slice()[0].byte_offset, 0);
        let allocated = lir::Value::Temp(call.direct_out().unwrap());
        assert!(matches!(instructions[1], lir::Instruction::HeapStore {
            object, offset: 16, value: lir::Value::Param(0)
        } if *object == allocated));
        assert_eq!(instructions.len(), 2);
        assert!(matches!(function.blocks[function.entry].terminator,
            lir::Terminator::Return { value: Some(value) } if value == allocated));
    }
}

#[test]
fn snapshot_checks_current_before_reading_the_shared_root_layout() {
    for mode in [lir::OptimizationMode::Debug, lir::OptimizationMode::Release] {
        let task =
            mir::ContextStorageType::new(ConeIdentity::SINGLE_FILE, mir::ContextStorageRole::Task);
        let node =
            mir::ContextStorageType::new(ConeIdentity::SINGLE_FILE, mir::ContextStorageRole::Node);
        let mut builder = Builder::new();
        let snapshot = builder.user_fn_body(
            "snapshot",
            vec![],
            mir::Type::Context(node),
            returning_body(
                Arena::new(),
                expr(
                    mir::Type::Context(node),
                    mir::ExprKind::Context(mir::ContextOperation::Snapshot),
                ),
            ),
        );
        let mut module = builder.finish(snapshot);
        retain_context_metadata(&mut module, [task, node]);
        let output = lower_test_input_with_optimization(&seal_strong_input(module), mode).unwrap();
        let function = &output.module().functions[0];
        let instructions = instructions_without_polls(&function.blocks[function.entry]);
        let lir::Instruction::TaskContextLoad { out } = instructions[0] else {
            panic!("snapshot reads the current Context once");
        };
        let current = lir::Value::Temp(*out);
        let lir::Instruction::Store {
            local: root,
            value: lir::Value::NullPointer(lir::PointerKind::Managed),
        } = instructions[1]
        else {
            panic!("a missing Context has an empty snapshot");
        };
        let lir::Instruction::BinOp {
            out: condition,
            op: lir::BinOp::Ne,
            lhs,
            rhs: lir::Value::NullPointer(lir::PointerKind::Managed),
        } = instructions[2]
        else {
            panic!("snapshot must guard the root load");
        };
        assert_eq!(*lhs, current);
        let lir::Terminator::CondBr {
            cond,
            then_block,
            else_block,
        } = function.blocks[function.entry].terminator
        else {
            panic!("snapshot must branch before the heap access");
        };
        assert_eq!(cond, lir::Value::Temp(*condition));
        let reads = instructions_without_polls(&function.blocks[then_block]);
        let lir::Instruction::HeapLoad {
            out,
            object,
            offset: 16,
        } = reads[0]
        else {
            panic!("snapshot uses the existing TaskContext root offset");
        };
        assert_eq!(*object, current);
        assert!(
            matches!(reads[1], lir::Instruction::Store { local, value } if local == root && *value == lir::Value::Temp(*out))
        );
        assert_eq!(reads.len(), 2);
        assert!(
            matches!(function.blocks[then_block].terminator, lir::Terminator::Br(target) if target == else_block)
        );
        assert!(
            matches!(function.blocks[else_block].terminator, lir::Terminator::Return { value: Some(lir::Value::Local(local)) } if local == *root)
        );
    }
}

fn retain_context_metadata(module: &mut mir::Module, types: [mir::ContextStorageType; 2]) {
    module.meta.source_exact_types = mir::SourceExactTypeIdentities::checked(
        module
            .meta
            .source_exact_types
            .iter()
            .filter(|entry| !matches!(entry.ty(), mir::Type::Context(_)))
            .cloned()
            .collect(),
    )
    .unwrap();
    module.meta.generated_exact_types = mir::GeneratedExactTypeIdentities::checked(
        types
            .into_iter()
            .map(|storage| {
                mir::GeneratedExactTypeIdentity::new(
                    mir::GeneratedExactTypeLocation::Context(storage),
                    &storage.nominal_record(),
                    None,
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
}
