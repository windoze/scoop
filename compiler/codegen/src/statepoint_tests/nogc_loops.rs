use super::*;
use inkwell::basic_block::BasicBlock;
use inkwell::values::{InstructionOpcode, Operand};
use scoop_lir::OptimizationMode;

#[test]
fn loop_load_forwarding_small_unroll_and_unswitch_require_nogc() {
    for effect in [GcEffect::NoGc, GcEffect::Managed] {
        let context = Context::create();
        let profile = ValidatedBackendProfile::darwin_aarch64_for_test()
            .with_optimization(OptimizationMode::Release);
        let machine = profile.create_target_machine().unwrap();
        let llvm = parse(&context, include_str!("nogc_loops.ll"));
        llvm.set_triple(&machine.get_triple());
        llvm.set_data_layout(&machine.get_target_data().get_data_layout());
        let expected = ExpectedSafepoints {
            sites: BTreeMap::new(),
            functions: ["prefix", "small", "unswitch"]
                .map(|name| (name.to_string(), effect))
                .into(),
        };
        let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
        assert_eq!(plan.site_count(), 0);
        llvm.verify().unwrap();
        let ir = llvm.print_to_string().to_string();

        let prefix = llvm.get_function("prefix").unwrap();
        let load_blocks = prefix
            .get_basic_blocks()
            .into_iter()
            .filter(|block| {
                block
                    .get_instructions()
                    .any(|instruction| instruction.get_opcode() == InstructionOpcode::Load)
            })
            .collect::<Vec<_>>();
        assert_eq!(load_blocks.len(), 1, "{ir}");
        assert_eq!(
            in_cycle(load_blocks[0]),
            effect == GcEffect::Managed,
            "{ir}"
        );

        let small = llvm.get_function("small").unwrap();
        assert_eq!(
            small.get_basic_blocks().into_iter().any(in_cycle),
            effect == GcEffect::Managed,
            "{ir}"
        );

        let unswitch = llvm.get_function("unswitch").unwrap();
        let flag_branches = unswitch
            .get_basic_blocks()
            .into_iter()
            .filter(|block| {
                block.get_terminator().is_some_and(|terminator| {
                    terminator.get_operand(0)
                        == Some(Operand::Value(unswitch.get_first_param().unwrap()))
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(flag_branches.len(), 1, "{ir}");
        assert_eq!(
            in_cycle(flag_branches[0]),
            effect == GcEffect::Managed,
            "{ir}"
        );
        machine
            .write_to_memory_buffer(&llvm, inkwell::targets::FileType::Object)
            .unwrap();
    }
}

fn in_cycle(start: BasicBlock<'_>) -> bool {
    let mut pending = successors(start);
    let mut visited = Vec::new();
    while let Some(block) = pending.pop() {
        if block == start {
            return true;
        }
        if !visited.contains(&block) {
            visited.push(block);
            pending.extend(successors(block));
        }
    }
    false
}

fn successors(block: BasicBlock<'_>) -> Vec<BasicBlock<'_>> {
    block
        .get_terminator()
        .unwrap()
        .get_operands()
        .flatten()
        .filter_map(|operand| match operand {
            Operand::Block(block) => Some(block),
            Operand::Value(_) => None,
        })
        .collect()
}
