use super::*;

/// Canonicalize the final LIR CFG before assigning any safepoint identity.
/// MIR lowering can leave detached EH and coroutine continuation blocks after
/// control-flow simplification. LLVM is allowed to delete those blocks, so
/// retaining them would make the supposedly exact LIR statepoint manifest
/// describe code that cannot occur in the emitted image.
pub(super) fn prune_unreachable_blocks(lowered: &mut LoweredFunction) {
    let LoweredFunction {
        function,
        loop_header_polls,
        ..
    } = lowered;
    let mut reachable = vec![false; function.blocks.len()];
    let mut worklist = vec![function.entry];
    while let Some(block_id) = worklist.pop() {
        let index = arena_index(block_id);
        if reachable[index] {
            continue;
        }
        reachable[index] = true;
        worklist.extend(block_successors(&function.blocks[block_id]));
    }

    if reachable.iter().all(|reachable| *reachable) {
        return;
    }

    let old_blocks = std::mem::take(&mut function.blocks);
    let mut new_blocks = Arena::with_capacity(reachable.iter().filter(|value| **value).count());
    let mut block_map = vec![None; reachable.len()];
    for (old_id, block) in old_blocks {
        if reachable[arena_index(old_id)] {
            let new_id = new_blocks.alloc(block);
            block_map[arena_index(old_id)] = Some(new_id);
        }
    }

    function.entry = remap_block(function.entry, &block_map);
    for block in new_blocks.values_mut() {
        remap_block_targets(block, &block_map);
    }
    *loop_header_polls = loop_header_polls
        .iter()
        .filter_map(|target| {
            block_map[arena_index(target.block())].map(MappedLoopHeaderPollTarget::new)
        })
        .collect();
    function.blocks = new_blocks;
}

pub(super) fn remap_block(old: lir::BlockId, block_map: &[Option<lir::BlockId>]) -> lir::BlockId {
    block_map[arena_index(old)].expect("every edge from a reachable block is reachable")
}

pub(super) fn remap_block_targets(block: &mut lir::BasicBlock, block_map: &[Option<lir::BlockId>]) {
    match &mut block.terminator {
        lir::Terminator::Br(target) => *target = remap_block(*target, block_map),
        lir::Terminator::CondBr {
            then_block,
            else_block,
            ..
        } => {
            *then_block = remap_block(*then_block, block_map);
            *else_block = remap_block(*else_block, block_map);
        }
        lir::Terminator::Return { .. }
        | lir::Terminator::Resume { .. }
        | lir::Terminator::Unreachable => {}
    }

    for instruction in &mut block.instructions {
        let lir::Instruction::Invoke { site } = instruction else {
            continue;
        };
        match site {
            lir::InvokeSite::Managed(site) => {
                site.normal = remap_block(site.normal, block_map);
                site.unwind = remap_block(site.unwind, block_map);
            }
            lir::InvokeSite::NoGc(site) => {
                site.normal = remap_block(site.normal, block_map);
                site.unwind = remap_block(site.unwind, block_map);
            }
        }
    }
}

pub(super) fn insert_polls(
    function: &mut lir::Function,
    loop_header_polls: &[MappedLoopHeaderPollTarget],
    sites: &mut PendingSafepointSites,
) {
    if function.gc_effect == lir::GcEffect::NoGc {
        return;
    }
    let mut poll_blocks = vec![false; function.blocks.len()];
    poll_blocks[arena_index(function.entry)] = true;
    for target in loop_header_polls {
        poll_blocks[arena_index(target.block())] = true;
    }
    let signature = function
        .call_targets
        .void_signatures
        .alloc(lir::VoidCallSignature::new(
            Vec::new(),
            lir::CallingConvention::Cdecl,
        ));
    let poll_target = function
        .call_targets
        .managed_targets
        .void
        .alloc(lir::CallTarget {
            destination: lir::ManagedCallDestination::runtime(
                lir::ManagedRuntimeFunction::Safepoint,
            ),
            signature,
        });
    for (block_id, block) in function.blocks.iter_mut() {
        if !poll_blocks[arena_index(block_id)] {
            continue;
        }
        let insertion = usize::from(matches!(
            block.instructions.first(),
            Some(lir::Instruction::LandingPad { .. } | lir::Instruction::CleanupPad { .. })
        ));
        block.instructions.insert(
            insertion,
            lir::Instruction::ManagedPoll {
                site: lir::ManagedPollSite {
                    target: poll_target,
                    safepoint: sites.allocate(lir::SafepointSiteRole::ManagedPoll),
                    live: lir::StatepointLiveSet::default(),
                },
            },
        );
    }
}
