use super::*;

#[derive(Debug, Clone)]
enum RootPlan {
    None,
    Statepoint(lir::StatepointLiveSet),
    Exceptional(lir::ExceptionalRootSet),
    NativeSafe(lir::NativeSafeRootSet),
    NativeBorrowed(Vec<lir::CallerRoot>),
}

pub(super) fn annotate_root_plans(
    context: &LoweringContext,
    function: &mut lir::Function,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) {
    let block_count = function.blocks.len();
    let mut uses = vec![HashSet::new(); block_count];
    let mut defs = vec![HashSet::new(); block_count];
    let mut successors = vec![Vec::new(); block_count];

    for (id, block) in function.blocks.iter() {
        let index = arena_index(id);
        let mut block_defs = HashSet::new();
        let mut block_uses = HashSet::new();
        for instruction in &block.instructions {
            for value in instruction_uses(instruction, function) {
                if let Some(value) = LiveValue::from_value(value)
                    && !block_defs.contains(&value)
                {
                    block_uses.insert(value);
                }
            }
            block_defs.extend(instruction_defs(instruction));
        }
        terminator_uses(&block.terminator, |value| {
            if let Some(value) = LiveValue::from_value(value)
                && !block_defs.contains(&value)
            {
                block_uses.insert(value);
            }
        });
        uses[index] = block_uses;
        defs[index] = block_defs;
        successors[index] = block_successors(block);
    }

    let mut live_in = vec![HashSet::new(); block_count];
    let mut live_out = vec![HashSet::new(); block_count];
    loop {
        let mut changed = false;
        for index in (0..block_count).rev() {
            let mut next_out = HashSet::new();
            for successor in &successors[index] {
                next_out.extend(live_in[arena_index(*successor)].iter().copied());
            }
            let mut next_in = uses[index].clone();
            next_in.extend(next_out.difference(&defs[index]).copied());
            if next_out != live_out[index] || next_in != live_in[index] {
                live_out[index] = next_out;
                live_in[index] = next_in;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut plans: Vec<Vec<RootPlan>> = function
        .blocks
        .iter()
        .map(|(_, block)| vec![RootPlan::None; block.instructions.len()])
        .collect();
    for (id, block) in function.blocks.iter() {
        let block_index = arena_index(id);
        let mut live = live_out[block_index].clone();
        terminator_uses(&block.terminator, |value| {
            if let Some(value) = LiveValue::from_value(value) {
                live.insert(value);
            }
        });
        for (instruction_index, instruction) in block.instructions.iter().enumerate().rev() {
            for definition in instruction_defs(instruction) {
                live.remove(&definition);
            }
            plans[block_index][instruction_index] = match instruction {
                lir::Instruction::ManagedPoll { .. } => RootPlan::Statepoint(statepoint_live_set(
                    context, &live, function, structs, enums,
                )),
                lir::Instruction::ArrayAlloc { elements, .. } => {
                    // Array allocation is expanded in codegen: element values
                    // remain live after the collecting slow-path call until
                    // they are stored into the new object.
                    let mut allocation_live = live.clone();
                    for element in elements {
                        if let Some(value) = LiveValue::from_value(*element)
                            && root_scan(context, live_value_ty(value, function), structs, enums, 0)
                                .contains_reference()
                        {
                            allocation_live.insert(value);
                        }
                    }
                    RootPlan::Statepoint(statepoint_live_set(
                        context,
                        &allocation_live,
                        function,
                        structs,
                        enums,
                    ))
                }
                lir::Instruction::ArrayAssembly { parts, .. } => {
                    let mut allocation_live = live.clone();
                    include_managed_operands(
                        context,
                        &mut allocation_live,
                        parts.iter().map(|part| match part {
                            lir::ArrayAssemblyPart::Element(value)
                            | lir::ArrayAssemblyPart::CopyArray(value) => *value,
                        }),
                        function,
                        structs,
                        enums,
                    );
                    RootPlan::Statepoint(statepoint_live_set(
                        context,
                        &allocation_live,
                        function,
                        structs,
                        enums,
                    ))
                }
                lir::Instruction::BoxValue { payload, .. } => {
                    let mut roots = live.clone();
                    include_managed_operands(
                        context,
                        &mut roots,
                        payload.source().map(lir::Value::Local),
                        function,
                        structs,
                        enums,
                    );
                    RootPlan::Statepoint(statepoint_live_set(
                        context, &roots, function, structs, enums,
                    ))
                }
                lir::Instruction::ArrayClone { operand, .. } => {
                    let mut roots = live.clone();
                    include_managed_operands(
                        context,
                        &mut roots,
                        [*operand],
                        function,
                        structs,
                        enums,
                    );
                    RootPlan::Statepoint(statepoint_live_set(
                        context, &roots, function, structs, enums,
                    ))
                }
                lir::Instruction::Call { site } => match site {
                    lir::CallSite::Managed(_) => {
                        let mut roots = live.clone();
                        include_managed_operands(
                            context,
                            &mut roots,
                            site.args().iter().map(|argument| argument.logical_value()),
                            function,
                            structs,
                            enums,
                        );
                        RootPlan::Statepoint(statepoint_live_set(
                            context, &roots, function, structs, enums,
                        ))
                    }
                    lir::CallSite::NoGc(_) => RootPlan::None,
                    lir::CallSite::NativeSafe(_) => {
                        RootPlan::NativeSafe(lir::NativeSafeRootSet::new(caller_roots(
                            context, &live, function, structs, enums,
                        )))
                    }
                    lir::CallSite::NativeBorrowed(site) => {
                        let mut borrowed_live = live.clone();
                        for argument in site.call.args() {
                            if let Some(value) = LiveValue::from_value(argument.logical_value())
                                && root_scan(
                                    context,
                                    live_value_ty(value, function),
                                    structs,
                                    enums,
                                    0,
                                )
                                .contains_reference()
                            {
                                borrowed_live.insert(value);
                            }
                        }
                        RootPlan::NativeBorrowed(caller_roots(
                            context,
                            &borrowed_live,
                            function,
                            structs,
                            enums,
                        ))
                    }
                },
                lir::Instruction::Invoke { site } => match site {
                    lir::InvokeSite::Managed(site) => RootPlan::Exceptional(exceptional_root_set(
                        context,
                        instruction,
                        site,
                        &live_in,
                        function,
                        structs,
                        enums,
                    )),
                    lir::InvokeSite::NoGc(_) => RootPlan::None,
                },
                lir::Instruction::NativeGlobalLoad { .. }
                | lir::Instruction::NativeGlobalStore { .. }
                | lir::Instruction::NativeGlobalAddress { .. } => {
                    RootPlan::NativeSafe(lir::NativeSafeRootSet::new(caller_roots(
                        context, &live, function, structs, enums,
                    )))
                }
                _ => RootPlan::None,
            };
            for value in instruction_uses(instruction, function) {
                if let Some(value) = LiveValue::from_value(value) {
                    live.insert(value);
                }
            }
        }
    }

    for (id, block) in function.blocks.iter_mut() {
        let block_index = arena_index(id);
        for (instruction_index, instruction) in block.instructions.iter_mut().enumerate() {
            let plan =
                std::mem::replace(&mut plans[block_index][instruction_index], RootPlan::None);
            match (instruction, plan) {
                (lir::Instruction::ManagedPoll { site }, RootPlan::Statepoint(live)) => {
                    site.live = live;
                }
                (
                    lir::Instruction::BoxValue {
                        live: instruction_live,
                        ..
                    }
                    | lir::Instruction::ArrayAlloc {
                        live: instruction_live,
                        ..
                    }
                    | lir::Instruction::ArrayAssembly {
                        live: instruction_live,
                        ..
                    }
                    | lir::Instruction::ArrayClone {
                        live: instruction_live,
                        ..
                    },
                    RootPlan::Statepoint(live),
                ) => *instruction_live = live,
                (
                    lir::Instruction::Call {
                        site: lir::CallSite::Managed(site),
                    },
                    RootPlan::Statepoint(live),
                ) => site.live = live,
                (
                    lir::Instruction::Call {
                        site: lir::CallSite::NativeSafe(site),
                    },
                    RootPlan::NativeSafe(roots),
                ) => site.roots = roots,
                (
                    lir::Instruction::Call {
                        site: lir::CallSite::NativeBorrowed(site),
                    },
                    RootPlan::NativeBorrowed(roots),
                ) => site.roots = lir::NativeBorrowedRootSet::new(roots),
                (
                    lir::Instruction::Invoke {
                        site: lir::InvokeSite::Managed(site),
                    },
                    RootPlan::Exceptional(roots),
                ) => site.roots = roots,
                (
                    lir::Instruction::NativeGlobalLoad { roots, .. }
                    | lir::Instruction::NativeGlobalStore { roots, .. }
                    | lir::Instruction::NativeGlobalAddress { roots, .. },
                    RootPlan::NativeSafe(computed),
                ) => *roots = computed,
                (
                    lir::Instruction::Call {
                        site: lir::CallSite::NoGc(_),
                    },
                    RootPlan::None,
                )
                | (
                    lir::Instruction::Invoke {
                        site: lir::InvokeSite::NoGc(_),
                    },
                    RootPlan::None,
                )
                | (_, RootPlan::None) => {}
                (_, unexpected) => {
                    panic!("root-plan kind does not match LIR instruction: {unexpected:?}")
                }
            }
        }
    }
}
