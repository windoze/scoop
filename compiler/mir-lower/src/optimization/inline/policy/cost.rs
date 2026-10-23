//! Keep cold-path frequency separate from actual inline growth.

use std::collections::{HashMap, HashSet};

use super::{copy_cost, mir};

#[derive(Clone, Copy)]
pub(in crate::optimization::inline) struct Cost {
    pub(in crate::optimization::inline) execution: usize,
    pub(in crate::optimization::inline) size: usize,
}

impl Cost {
    pub(in crate::optimization::inline) fn with_parameters(
        mut self,
        structs: &la_arena::Arena<mir::StructDef>,
        function: &mir::Function,
    ) -> Self {
        let parameters = function
            .params
            .iter()
            .map(|parameter| assignment_cost(structs, &parameter.ty))
            .sum::<usize>();
        self.execution += parameters;
        self.size += parameters;
        self
    }
}

pub(in crate::optimization::inline) fn function_cost(
    structs: &la_arena::Arena<mir::StructDef>,
    function: &mir::Function,
) -> Cost {
    blocks_cost(structs, function.body.blocks.iter()).with_parameters(structs, function)
}

pub(in crate::optimization::inline) fn blocks_cost<'a>(
    structs: &la_arena::Arena<mir::StructDef>,
    blocks: impl Iterator<Item = (mir::BlockId, &'a mir::BasicBlock)>,
) -> Cost {
    adjusted_blocks_cost(structs, blocks, |_, _, size| size)
}

pub(in crate::optimization::inline) fn adjusted_blocks_cost<'a>(
    structs: &la_arena::Arena<mir::StructDef>,
    blocks: impl Iterator<Item = (mir::BlockId, &'a mir::BasicBlock)>,
    mut adjust: impl FnMut(mir::BlockId, &mir::BasicBlock, usize) -> usize,
) -> Cost {
    let blocks = blocks.collect::<Vec<_>>();
    let cold = cold_blocks(&blocks);
    let mut cost = Cost {
        execution: 0,
        size: 0,
    };
    for (id, block) in blocks {
        let size = adjust(id, block, block_cost(structs, block));
        cost.size += size;
        cost.execution += if cold.contains(&id) {
            size.div_ceil(4)
        } else {
            size
        };
    }
    cost
}

fn cold_blocks(blocks: &[(mir::BlockId, &mir::BasicBlock)]) -> HashSet<mir::BlockId> {
    let mut cold = HashSet::new();
    if !blocks
        .iter()
        .any(|(_, block)| matches!(block.terminator, mir::Terminator::Return { .. }))
    {
        return cold;
    }
    let mut incoming = HashMap::<_, Vec<_>>::new();
    let mut remaining = HashMap::new();
    let mut pending = Vec::new();
    for &(id, block) in blocks {
        let successors = match block.terminator {
            mir::Terminator::Throw { unwind: None, .. }
            | mir::Terminator::Rethrow { unwind: None }
            | mir::Terminator::Resume
            | mir::Terminator::Trap { .. } => {
                cold.insert(id);
                pending.push(id);
                Vec::new()
            }
            mir::Terminator::Goto(target) => vec![target],
            mir::Terminator::Branch {
                then_block,
                else_block,
                ..
            } => vec![then_block, else_block],
            _ => Vec::new(),
        };
        if !successors.is_empty() {
            remaining.insert(id, successors.len());
            for successor in successors {
                incoming.entry(successor).or_default().push(id);
            }
        }
    }
    while let Some(id) = pending.pop() {
        for &predecessor in incoming.get(&id).into_iter().flatten() {
            let count = remaining.get_mut(&predecessor).unwrap();
            *count -= 1;
            if *count == 0 && cold.insert(predecessor) {
                pending.push(predecessor);
            }
        }
    }
    cold
}

fn block_cost(structs: &la_arena::Arena<mir::StructDef>, block: &mir::BasicBlock) -> usize {
    let mut cost = usize::from(block.unwind.is_some()) * 4;
    for statement in &block.statements {
        cost += statement_cost(structs, statement);
    }
    mir::visit_block_exprs(block, &mut |expression| {
        cost += expression_cost(expression);
    });
    cost + match block.terminator {
        mir::Terminator::Goto(_) | mir::Terminator::Unreachable => 0,
        mir::Terminator::Branch { .. } => 2,
        mir::Terminator::Throw { .. }
        | mir::Terminator::Rethrow { .. }
        | mir::Terminator::Resume => 8,
        _ => 1,
    }
}

pub(in crate::optimization::inline) fn assignment_cost(
    structs: &la_arena::Arena<mir::StructDef>,
    ty: &mir::Type,
) -> usize {
    1 + copy_cost(structs, ty)
}

pub(in crate::optimization::inline) fn statement_cost(
    structs: &la_arena::Arena<mir::StructDef>,
    statement: &mir::Statement,
) -> usize {
    match &statement.kind {
        mir::StatementKind::Call(effect) => {
            if matches!(
                crate::optimization::call(effect).target,
                mir::CallTarget::Direct(_)
            ) {
                6
            } else {
                10
            }
        }
        mir::StatementKind::ValDecl { init, .. }
        | mir::StatementKind::Assign { value: init, .. } => assignment_cost(structs, &init.ty),
        mir::StatementKind::Eh(_) => 8,
        _ => 2,
    }
}

pub(in crate::optimization::inline) fn expression_cost(expression: &mir::Expr) -> usize {
    match expression.kind {
        mir::ExprKind::Local(_)
        | mir::ExprKind::IntegerLiteral(_)
        | mir::ExprKind::BoolLiteral(_)
        | mir::ExprKind::CharLiteral(_)
        | mir::ExprKind::FloatLiteral(_)
        | mir::ExprKind::UnitLiteral
        | mir::ExprKind::StringConst(_)
        | mir::ExprKind::MachineScalarLiteral(_) => 0,
        mir::ExprKind::ClassAlloc { .. }
        | mir::ExprKind::ClosureAlloc { .. }
        | mir::ExprKind::Box(_)
        | mir::ExprKind::ArrayAllocate { .. }
        | mir::ExprKind::ArrayLiteral { .. }
        | mir::ExprKind::ArrayAssembly { .. }
        | mir::ExprKind::ArrayClone { .. }
        | mir::ExprKind::AtomicNew(_) => 12,
        mir::ExprKind::Context(_) | mir::ExprKind::Atomic(_) => 6,
        mir::ExprKind::StructInit { .. }
        | mir::ExprKind::StructConstruct { .. }
        | mir::ExprKind::TupleLiteral(_)
        | mir::ExprKind::VariantConstruct { .. } => 4,
        mir::ExprKind::FieldAccess { .. }
        | mir::ExprKind::ArrayGet { .. }
        | mir::ExprKind::PtrLoad { .. } => 2,
        _ => 1,
    }
}

pub(in crate::optimization::inline) fn tree_cost(expression: &mir::Expr) -> usize {
    let mut cost = 0;
    mir::visit_expr(expression, &mut |expr| cost += expression_cost(expr));
    cost
}
