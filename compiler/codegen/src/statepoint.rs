//! LLVM statepoint policy and safepoint placement for managed functions.
//!
//! This module owns the boundary between typed LIR GC effects and LLVM's
//! statepoint machinery.  Mechanical instruction emission remains in the
//! parent codegen module.

use std::collections::HashSet;

use inkwell::module::Module as LlvmModule;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::TargetMachine;
use inkwell::values::FunctionValue;
use la_arena::Idx;
use scoop_lir::{Function, GcEffect, Instruction, Terminator};

use crate::CodegenError;

/// LLVM's built-in statepoint strategy used by Scoop managed functions.
const GC_STRATEGY: &str = "statepoint-example";

/// Runtime poll called at managed entries and loop headers.
pub(crate) const SAFEPOINT_SYMBOL: &str = "scoop_rt_safepoint";

/// Apply the LLVM GC strategy selected by the typed LIR effect.
pub(crate) fn configure_function(function: FunctionValue<'_>, effect: GcEffect) {
    if effect == GcEffect::Managed {
        function.set_gc(GC_STRATEGY);
    }
}

/// Rewrite calls in managed functions to LLVM statepoints.
pub(crate) fn rewrite(
    module: &LlvmModule<'_>,
    machine: &TargetMachine,
) -> Result<(), CodegenError> {
    module
        .run_passes(
            "rewrite-statepoints-for-gc",
            machine,
            PassBuilderOptions::create(),
        )
        .map_err(|error| CodegenError(format!("rewrite-statepoints-for-gc failed: {error}")))
}

/// Return a dense set of LIR blocks that are targets of back edges.
///
/// An edge B -> T is a back edge when T dominates B.  The graph includes
/// branch terminators and the unwind edges of invokes.  LIR lowering's loop
/// shape makes these blocks the `while.cond` blocks in normal source code.
pub(crate) fn loop_headers(function: &Function) -> Vec<bool> {
    let len = function.blocks.len();
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); len];
    for (id, block) in function.blocks.iter() {
        let from = arena_index(id);
        match &block.terminator {
            Terminator::Br(target) => successors[from].push(arena_index(*target)),
            Terminator::CondBr {
                then_block,
                else_block,
                ..
            } => {
                successors[from].push(arena_index(*then_block));
                successors[from].push(arena_index(*else_block));
            }
            Terminator::Return { .. } | Terminator::Resume { .. } | Terminator::Unreachable => {}
        }
        // Invoke is terminator-like: the block terminator repeats its normal
        // successor, so only the unwind edge is additional here.
        if let Some(Instruction::Invoke { unwind, .. }) = block.instructions.last() {
            successors[from].push(arena_index(*unwind));
        }
    }

    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); len];
    for (from, targets) in successors.iter().enumerate() {
        for &to in targets {
            predecessors[to].push(from);
        }
    }

    let entry = arena_index(function.entry);
    let mut dominators: Vec<HashSet<usize>> = vec![(0..len).collect(); len];
    dominators[entry] = [entry].into_iter().collect();
    let mut changed = true;
    while changed {
        changed = false;
        for block in 0..len {
            if block == entry {
                continue;
            }
            let mut dom: HashSet<usize> = match predecessors[block].as_slice() {
                [] => [block].into_iter().collect(),
                [first, rest @ ..] => {
                    let mut dom = dominators[*first].clone();
                    for pred in rest {
                        dom.retain(|candidate| dominators[*pred].contains(candidate));
                    }
                    dom
                }
            };
            dom.insert(block);
            if dom != dominators[block] {
                dominators[block] = dom;
                changed = true;
            }
        }
    }

    let mut headers = vec![false; len];
    for (from, targets) in successors.iter().enumerate() {
        for &to in targets {
            if dominators[from].contains(&to) {
                headers[to] = true;
            }
        }
    }
    headers
}

fn arena_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
