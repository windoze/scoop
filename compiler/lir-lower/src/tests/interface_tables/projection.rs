use super::*;

#[test]
fn inherited_calls_project_directly_in_every_profile_without_aggregate_temporaries() {
    for mode in [lir::OptimizationMode::Debug, lir::OptimizationMode::Release] {
        let (module, scope) = fixture_type(true);
        let lowered = physical(module, mode);
        let function = function(&lowered, scope);
        assert_eq!(lookup_count(function), 0);
        assert_eq!(function.locals.len(), 1);
        assert!(
            !function
                .blocks
                .iter()
                .flat_map(|(_, block)| &block.instructions)
                .any(|instruction| matches!(
                    instruction,
                    lir::Instruction::MakeAggregate { out, .. } if function.temps[*out].ty == lir::LirType::Interface
                ))
        );
        let roots = function
            .blocks
            .iter()
            .flat_map(|(_, block)| &block.instructions)
            .filter_map(|instruction| match instruction {
                lir::Instruction::Call {
                    site: lir::CallSite::Managed(site),
                } => Some(site.live.as_slice().len()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(roots, vec![2, 1]);
    }
}

#[test]
fn real_parent_arguments_use_the_prefix_or_a_fixed_pointer_adjustment() {
    for parent_name in ["Parent", "Other"] {
        for mode in [lir::OptimizationMode::Debug, lir::OptimizationMode::Release] {
            let (mut module, scope) = fixture_type(true);
            let parent = module
                .interfaces
                .iter()
                .find(|(_, interface)| interface.name == parent_name)
                .unwrap()
                .0;
            let callee = module
                .functions
                .iter()
                .find(|(_, function)| function.name == format!("{parent_name}.read"))
                .unwrap()
                .0;
            let function = &mut module.functions[scope];
            let source = current(function);
            let argument = retype(
                local_expr(source, function.body.locals[source].ty.clone()),
                mir::Type::Interface(parent),
            );
            function.body.blocks[function.body.entry].statements[1] =
                stmt(mir::StatementKind::Call(mir::CallEffect::Unit(mir::Call {
                    target: mir::CallTarget::Direct(mir::Callee::User(callee)),
                    args: vec![argument],
                })));
            let lowered = physical(module, mode);
            let function = super::function(&lowered, scope);
            assert_eq!(lookup_count(function), 0);
            let instructions = function
                .blocks
                .iter()
                .flat_map(|(_, block)| &block.instructions)
                .collect::<Vec<_>>();
            assert_eq!(
                instructions
                    .iter()
                    .filter(|instruction| matches!(
                        instruction,
                        lir::Instruction::MakeAggregate { out, .. } if function.temps[*out].ty == lir::LirType::Interface
                    ))
                    .count(),
                1
            );
            let offsets = instructions
                .iter()
                .filter_map(|instruction| match instruction {
                    lir::Instruction::PtrOffset {
                        element_offset,
                        element_size,
                        out,
                        ..
                    } => {
                        assert_eq!(function.temps[*out].ty, lir::METADATA_PTR);
                        assert_eq!(element_size.get(), 8);
                        Some(*element_offset)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                offsets,
                if parent_name == "Parent" {
                    vec![]
                } else {
                    vec![lir::Value::MachineScalar(
                        lir::MachineScalarValue::PointerElementOffset(1),
                    )]
                }
            );
        }
    }
}
