//! Recompute call-site roots independently from the supplied LIR plans.
//!
//! Emission deliberately consumes protocol-specific plans mechanically. This
//! boundary validator therefore derives CFG liveness and canonical scans from
//! the function itself, so an incomplete plan cannot validate itself through
//! the statepoint manifest or caller-root publisher.

use std::collections::HashSet;

use super::scoop_abi::canonical_storage_scan;
use super::*;

mod dataflow;
use dataflow::*;

pub(super) fn validate_call_root_plans(module: &Module) -> Result<(), CodegenError> {
    for function in &module.functions {
        validate_function(module, function)?;
    }
    Ok(())
}

fn validate_function(module: &Module, function: &Function) -> Result<(), CodegenError> {
    let block_count = function.blocks.len();
    let mut uses = vec![HashSet::new(); block_count];
    let mut defs = vec![HashSet::new(); block_count];
    let mut successors = vec![Vec::new(); block_count];

    for (id, block) in function.blocks.iter() {
        validate_invoke_shape(function, block)?;
        let index = arena_index(id);
        let mut block_uses = HashSet::new();
        let mut block_defs = HashSet::new();
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
        let block_successors = block_successors(block);
        for successor in &block_successors {
            if arena_index(*successor) >= block_count {
                return Err(CodegenError(format!(
                    "call root-plan validation in @{} reached invalid block {}",
                    function.symbol(),
                    successor.into_raw()
                )));
            }
        }
        uses[index] = block_uses;
        defs[index] = block_defs;
        successors[index] = block_successors;
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

    for (block_id, block) in function.blocks.iter() {
        let mut live = live_out[arena_index(block_id)].clone();
        terminator_uses(&block.terminator, |value| {
            if let Some(value) = LiveValue::from_value(value) {
                live.insert(value);
            }
        });
        for (instruction_index, instruction) in block.instructions.iter().enumerate().rev() {
            for definition in instruction_defs(instruction) {
                live.remove(&definition);
            }
            match instruction {
                Instruction::ManagedPoll { site } => validate_statepoint_roots(
                    module,
                    function,
                    &site_owner(function, block_id, instruction_index, "managed poll"),
                    site.live.as_slice(),
                    &live,
                )?,
                Instruction::Call { site } => {
                    validate_call_plan(module, function, block_id, instruction_index, site, &live)?
                }
                Instruction::Invoke {
                    site: scoop_lir::InvokeSite::Managed(site),
                } => validate_invoke_plan(
                    module,
                    function,
                    block_id,
                    instruction_index,
                    instruction,
                    site,
                    &live_in,
                )?,
                Instruction::Invoke {
                    site: scoop_lir::InvokeSite::NoGc(_),
                } => {}
                _ => {}
            }
            for value in instruction_uses(instruction, function) {
                if let Some(value) = LiveValue::from_value(value) {
                    live.insert(value);
                }
            }
        }
    }
    Ok(())
}

fn validate_invoke_shape(
    function: &Function,
    block: &scoop_lir::BasicBlock,
) -> Result<(), CodegenError> {
    let block_count = function.blocks.len();
    for (index, instruction) in block.instructions.iter().enumerate() {
        let Instruction::Invoke { site } = instruction else {
            continue;
        };
        if index + 1 != block.instructions.len() {
            return Err(CodegenError(format!(
                "invoke @{}: must be the last instruction of block {}",
                function.symbol(),
                block.name
            )));
        }
        for successor in [site.normal(), site.unwind()] {
            if arena_index(successor) >= block_count {
                return Err(CodegenError(format!(
                    "call root-plan validation in @{} reached invalid block {}",
                    function.symbol(),
                    successor.into_raw()
                )));
            }
        }
        match &block.terminator {
            Terminator::Br(target) if *target == site.normal() => {}
            _ => {
                return Err(CodegenError(format!(
                    "invoke block @{}:{}: terminator must be `br` to the invoke's normal target",
                    function.symbol(),
                    block.name
                )));
            }
        }
    }
    Ok(())
}

fn validate_call_plan(
    module: &Module,
    function: &Function,
    block: scoop_lir::BlockId,
    instruction: usize,
    site: &scoop_lir::CallSite,
    live_after_defs: &HashSet<LiveValue>,
) -> Result<(), CodegenError> {
    match site {
        scoop_lir::CallSite::Managed(site) => {
            let mut expected = live_after_defs.clone();
            include_managed_operands(module, function, &mut expected, site.call.args())?;
            validate_statepoint_roots(
                module,
                function,
                &site_owner(function, block, instruction, "managed call"),
                site.live.as_slice(),
                &expected,
            )
        }
        scoop_lir::CallSite::NoGc(_) => Ok(()),
        scoop_lir::CallSite::NativeSafe(site) => validate_caller_roots(
            module,
            function,
            &site_owner(function, block, instruction, "native-safe call"),
            site.roots.as_slice(),
            live_after_defs,
        ),
        scoop_lir::CallSite::NativeBorrowed(site) => {
            let mut expected = live_after_defs.clone();
            include_managed_operands(module, function, &mut expected, site.call.args())?;
            validate_caller_roots(
                module,
                function,
                &site_owner(function, block, instruction, "native-borrowed call"),
                site.roots.as_slice(),
                &expected,
            )
        }
    }
}

fn validate_invoke_plan(
    module: &Module,
    function: &Function,
    block: scoop_lir::BlockId,
    instruction_index: usize,
    instruction: &Instruction,
    site: &scoop_lir::ManagedInvokeSite,
    live_in: &[HashSet<LiveValue>],
) -> Result<(), CodegenError> {
    let mut normal = live_in[arena_index(site.normal)].clone();
    for definition in instruction_defs(instruction) {
        normal.remove(&definition);
    }
    let unwind = &live_in[arena_index(site.unwind)];
    let mut expected = normal.union(unwind).copied().collect::<HashSet<_>>();
    include_managed_operands(module, function, &mut expected, site.call.args())?;
    validate_exceptional_roots(
        module,
        function,
        &site_owner(function, block, instruction_index, "managed invoke"),
        site.roots.as_slice(),
        &expected,
        &normal,
        unwind,
    )
}

fn include_managed_operands(
    module: &Module,
    function: &Function,
    roots: &mut HashSet<LiveValue>,
    arguments: &[scoop_lir::AbiCallArgument],
) -> Result<(), CodegenError> {
    for argument in arguments {
        let Some(value) = LiveValue::from_value(argument.logical_value()) else {
            continue;
        };
        if canonical_scan(module, function, value, "call argument root")?.contains_reference() {
            roots.insert(value);
        }
    }
    Ok(())
}

fn validate_statepoint_roots(
    module: &Module,
    function: &Function,
    owner: &str,
    actual: &[scoop_lir::StatepointLiveValue],
    expected_live: &HashSet<LiveValue>,
) -> Result<(), CodegenError> {
    let expected = canonical_roots(module, function, expected_live, owner)?;
    if actual.len() != expected.len() {
        return Err(root_count_error(owner, actual.len(), expected.len()));
    }
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        validate_source(owner, index, actual.source, expected.value.source())?;
        let expected_ty = live_value_type(function, expected.value, owner)?;
        if &actual.ty != expected_ty {
            return Err(CodegenError(format!(
                "{owner} root {} carries type {}, expected canonical {}",
                source_name(actual.source),
                actual.ty.dump(),
                expected_ty.dump()
            )));
        }
        let actual_offsets = actual
            .leaves
            .as_slice()
            .iter()
            .map(|leaf| leaf.byte_offset)
            .collect::<Vec<_>>();
        let expected_offsets = flattened_offsets(&expected.scan);
        if actual_offsets != expected_offsets {
            return Err(CodegenError(format!(
                "{owner} root {} has managed leaf offsets {actual_offsets:?}, expected canonical {expected_offsets:?}",
                source_name(actual.source)
            )));
        }
    }
    Ok(())
}

fn validate_caller_roots(
    module: &Module,
    function: &Function,
    owner: &str,
    actual: &[scoop_lir::CallerRoot],
    expected_live: &HashSet<LiveValue>,
) -> Result<(), CodegenError> {
    let expected = canonical_roots(module, function, expected_live, owner)?;
    if actual.len() != expected.len() {
        return Err(root_count_error(owner, actual.len(), expected.len()));
    }
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        validate_source(owner, index, actual.source, expected.value.source())?;
        validate_scan(
            owner,
            actual.source,
            actual.scan.as_ref_scan(),
            &expected.scan,
        )?;
    }
    Ok(())
}

fn validate_exceptional_roots(
    module: &Module,
    function: &Function,
    owner: &str,
    actual: &[scoop_lir::ExceptionalRoot],
    expected_live: &HashSet<LiveValue>,
    normal: &HashSet<LiveValue>,
    unwind: &HashSet<LiveValue>,
) -> Result<(), CodegenError> {
    let expected = canonical_roots(module, function, expected_live, owner)?;
    if actual.len() != expected.len() {
        return Err(root_count_error(owner, actual.len(), expected.len()));
    }
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let source = expected.value.source();
        validate_source(owner, index, actual.root.source, source)?;
        validate_scan(
            owner,
            actual.root.source,
            actual.root.scan.as_ref_scan(),
            &expected.scan,
        )?;
        let expected_normal = normal.contains(&expected.value);
        let expected_unwind = unwind.contains(&expected.value);
        if actual.normal_live != expected_normal || actual.unwind_live != expected_unwind {
            return Err(CodegenError(format!(
                "{owner} root {} has edge flags normal_live={}/unwind_live={}, expected {expected_normal}/{expected_unwind}",
                source_name(source),
                actual.normal_live,
                actual.unwind_live
            )));
        }
    }
    Ok(())
}

struct CanonicalRoot {
    value: LiveValue,
    scan: RefScan,
}

fn canonical_roots(
    module: &Module,
    function: &Function,
    live: &HashSet<LiveValue>,
    owner: &str,
) -> Result<Vec<CanonicalRoot>, CodegenError> {
    let mut values = live.iter().copied().collect::<Vec<_>>();
    values.sort_by_key(|value| value.sort_key());
    let mut roots = Vec::new();
    for value in values {
        let scan = canonical_scan(module, function, value, owner)?;
        if scan.contains_reference() {
            roots.push(CanonicalRoot { value, scan });
        }
    }
    Ok(roots)
}

fn canonical_scan(
    module: &Module,
    function: &Function,
    value: LiveValue,
    owner: &str,
) -> Result<RefScan, CodegenError> {
    canonical_storage_scan(module, live_value_type(function, value, owner)?, owner)
}

fn live_value_type<'a>(
    function: &'a Function,
    value: LiveValue,
    owner: &str,
) -> Result<&'a LirType, CodegenError> {
    match value {
        LiveValue::Param(index) => function
            .signature
            .arguments()
            .get(index as usize)
            .map(|argument| argument.logical_storage_type())
            .ok_or_else(|| {
                CodegenError(format!(
                    "{owner} references invalid root parameter {index} in @{}",
                    function.symbol()
                ))
            }),
        LiveValue::Local(id) => {
            let index = arena_index(id);
            if index >= function.locals.len() {
                return Err(CodegenError(format!(
                    "{owner} references invalid root local {index} in @{}",
                    function.symbol()
                )));
            }
            Ok(function.locals[id].ty())
        }
        LiveValue::Temp(id) => {
            let index = arena_index(id);
            if index >= function.temps.len() {
                return Err(CodegenError(format!(
                    "{owner} references invalid root temporary t{index} in @{}",
                    function.symbol()
                )));
            }
            Ok(&function.temps[id].ty)
        }
    }
}

fn validate_source(
    owner: &str,
    index: usize,
    actual: scoop_lir::CallerRootSource,
    expected: scoop_lir::CallerRootSource,
) -> Result<(), CodegenError> {
    if actual != expected {
        return Err(CodegenError(format!(
            "{owner} root entry {index} names {}, expected {}",
            source_name(actual),
            source_name(expected)
        )));
    }
    Ok(())
}

fn validate_scan(
    owner: &str,
    source: scoop_lir::CallerRootSource,
    actual: &RefScan,
    expected: &RefScan,
) -> Result<(), CodegenError> {
    if actual != expected {
        return Err(CodegenError(format!(
            "{owner} root {} has scan {}, expected canonical {}",
            source_name(source),
            actual.dump(),
            expected.dump()
        )));
    }
    Ok(())
}

fn root_count_error(owner: &str, actual: usize, expected: usize) -> CodegenError {
    CodegenError(format!(
        "{owner} root plan has {actual} entries, expected {expected} complete entries"
    ))
}

fn source_name(source: scoop_lir::CallerRootSource) -> String {
    match source {
        scoop_lir::CallerRootSource::Param(index) => format!("param{index}"),
        scoop_lir::CallerRootSource::Local(id) => format!("local{}", id.into_raw()),
        scoop_lir::CallerRootSource::Temp(id) => format!("t{}", id.into_raw()),
    }
}

fn site_owner(
    function: &Function,
    block: scoop_lir::BlockId,
    instruction: usize,
    protocol: &str,
) -> String {
    format!(
        "{protocol} root plan in @{} block{} instruction {instruction}",
        function.symbol(),
        block.into_raw()
    )
}

fn flattened_offsets(scan: &RefScan) -> Vec<u64> {
    fn collect(scan: &RefScan, offsets: &mut Vec<u64>) {
        match scan {
            RefScan::None => {}
            RefScan::References(references) => offsets.extend(references),
            RefScan::Sequence(parts) => {
                for part in parts {
                    collect(part, offsets);
                }
            }
            RefScan::Array { .. } => {
                unreachable!("root value scans cannot contain variable object scans")
            }
        }
    }

    let mut offsets = Vec::new();
    collect(scan, &mut offsets);
    offsets.sort_unstable();
    offsets.dedup();
    offsets
}
