use std::collections::{BTreeMap, HashSet};

use super::*;

pub(super) fn validate_safepoint_identities(module: &Module) -> Result<(), CodegenError> {
    let mut persistent_sites = BTreeMap::new();
    let mut runtime_sites = BTreeMap::new();
    for function in &module.functions {
        validate_function(function, &mut persistent_sites, &mut runtime_sites)?;
    }
    Ok(())
}

fn validate_function(
    function: &Function,
    persistent_sites: &mut BTreeMap<scoop_lir::PersistentSafepointSiteId, String>,
    runtime_sites: &mut BTreeMap<scoop_lir::SafepointId, scoop_lir::PersistentSafepointSiteId>,
) -> Result<(), CodegenError> {
    let mut ordinals = BTreeMap::<scoop_lir::SafepointSiteRole, usize>::new();
    let mut references = HashSet::new();
    for block in canonical_reverse_postorder(function)? {
        for instruction in &function.blocks[block].instructions {
            let Some((expected_role, reference)) = instruction.safepoint() else {
                continue;
            };
            if function.gc_effect == GcEffect::NoGc {
                return Err(CodegenError(format!(
                    "NoGc LIR function `{}` contains a safepoint",
                    function.symbol()
                )));
            }
            if !references.insert(reference) {
                return Err(CodegenError(format!(
                    "LIR function `{}` reuses safepoint reference {}",
                    function.symbol(),
                    reference.into_u32()
                )));
            }
            let identity = function.safepoints.get(reference).ok_or_else(|| {
                CodegenError(format!(
                    "LIR function `{}` references missing safepoint site {}",
                    function.symbol(),
                    reference.into_u32()
                ))
            })?;
            if identity.owner() != function.callable_body.id() {
                return Err(CodegenError(format!(
                    "LIR function `{}` safepoint site {} belongs to another callable body",
                    function.symbol(),
                    reference.into_u32()
                )));
            }
            if identity.role() != expected_role {
                return Err(CodegenError(format!(
                    "LIR function `{}` safepoint site {} has role {:?}, expected {:?}",
                    function.symbol(),
                    reference.into_u32(),
                    identity.role(),
                    expected_role
                )));
            }
            let next_ordinal = ordinals.entry(expected_role).or_default();
            let expected_ordinal = u32::try_from(*next_ordinal).map_err(|_| {
                CodegenError(format!(
                    "LIR function `{}` has more than u32::MAX {:?} safepoints",
                    function.symbol(),
                    expected_role
                ))
            })?;
            if identity.ordinal() != expected_ordinal {
                return Err(CodegenError(format!(
                    "LIR function `{}` safepoint site {} has {:?} ordinal {}, expected {}",
                    function.symbol(),
                    reference.into_u32(),
                    expected_role,
                    identity.ordinal(),
                    expected_ordinal
                )));
            }
            *next_ordinal = next_ordinal.checked_add(1).ok_or_else(|| {
                CodegenError(format!(
                    "LIR function `{}` has more than usize::MAX {:?} safepoints",
                    function.symbol(),
                    expected_role
                ))
            })?;
            let location = format!("{}:{}", function.symbol(), reference.into_u32());
            if let Some(first) = persistent_sites.insert(identity.site_id(), location.clone()) {
                return Err(CodegenError(format!(
                    "persistent safepoint site {} is defined by both `{first}` and `{location}`",
                    identity.site_id()
                )));
            }
            if let Some(first_site) =
                runtime_sites.insert(identity.runtime_id(), identity.site_id())
            {
                return Err(CodegenError(format!(
                    "runtime SafepointId {} maps to persistent sites {} and {}",
                    identity.runtime_id().get(),
                    first_site,
                    identity.site_id()
                )));
            }
        }
    }
    if references.len() != function.safepoints.len() {
        return Err(CodegenError(format!(
            "LIR function `{}` has {} referenced safepoints but {} identity records",
            function.symbol(),
            references.len(),
            function.safepoints.len()
        )));
    }
    Ok(())
}

fn canonical_reverse_postorder(
    function: &Function,
) -> Result<Vec<scoop_lir::BlockId>, CodegenError> {
    let mut visited = vec![false; function.blocks.len()];
    let mut postorder = Vec::with_capacity(function.blocks.len());
    let mut stack = vec![(function.entry, false)];
    while let Some((block, expanded)) = stack.pop() {
        let index = arena_index(block);
        if index >= function.blocks.len() {
            return Err(CodegenError(format!(
                "LIR function `{}` reaches invalid block {} while ordering safepoints",
                function.symbol(),
                block.into_raw()
            )));
        }
        if expanded {
            postorder.push(block);
            continue;
        }
        if visited[index] {
            continue;
        }
        visited[index] = true;
        stack.push((block, true));
        stack.extend(
            semantic_successors(function, &function.blocks[block])?
                .into_iter()
                .rev()
                .map(|successor| (successor, false)),
        );
    }
    if let Some(first) = visited.iter().position(|reachable| !reachable) {
        return Err(CodegenError(format!(
            "LIR function `{}` contains unreachable block {first} during safepoint validation",
            function.symbol()
        )));
    }
    postorder.reverse();
    Ok(postorder)
}

fn semantic_successors(
    function: &Function,
    block: &scoop_lir::BasicBlock,
) -> Result<Vec<scoop_lir::BlockId>, CodegenError> {
    if block.instructions[..block.instructions.len().saturating_sub(1)]
        .iter()
        .any(|instruction| matches!(instruction, Instruction::Invoke { .. }))
    {
        return Err(CodegenError(format!(
            "invoke @{}: must be the last instruction of block {}",
            function.symbol(),
            block.name
        )));
    }
    if let Some(Instruction::Invoke { site }) = block.instructions.last() {
        if !matches!(block.terminator, Terminator::Br(target) if target == site.normal()) {
            return Err(CodegenError(format!(
                "invoke block @{}:{}: terminator must be `br` to the invoke's normal target",
                function.symbol(),
                block.name
            )));
        }
        return Ok(vec![site.normal(), site.unwind()]);
    }
    Ok(match block.terminator {
        Terminator::Br(target) => vec![target],
        Terminator::CondBr {
            then_block,
            else_block,
            ..
        } => vec![then_block, else_block],
        Terminator::Return { .. } | Terminator::Resume { .. } | Terminator::Unreachable => {
            Vec::new()
        }
    })
}
