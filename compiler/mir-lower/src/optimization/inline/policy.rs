use super::super::{graph, mir};

pub(super) fn local_target(module: &mir::Module, call: &mir::Call) -> Option<mir::FunctionId> {
    match call.target {
        mir::CallTarget::Direct(mir::Callee::User(function)) => Some(function),
        mir::CallTarget::Direct(mir::Callee::Monomorphized(instance)) => {
            Some(module.meta.instances[instance].function)
        }
        _ => None,
    }
}

pub(super) fn recursive(module: &mir::Module) -> Vec<bool> {
    let edges = module
        .functions
        .iter()
        .map(|(_, function)| {
            function
                .body
                .blocks
                .iter()
                .flat_map(|(_, block)| &block.statements)
                .filter_map(|statement| match &statement.kind {
                    mir::StatementKind::Call(effect) => {
                        local_target(module, super::super::call(effect)).map(graph::index)
                    }
                    _ => None,
                })
                .collect()
        })
        .collect::<Vec<_>>();
    graph::cyclic_nodes(&edges)
}

pub(super) fn protocol_entries(module: &mir::Module) -> Vec<bool> {
    let mut excluded = vec![false; module.functions.len()];
    let mut exclude = |function| excluded[graph::index(function)] = true;
    for (_, callback) in module.callback_bridges.iter() {
        if let Some((_, bridge)) = callback.local_definition() {
            exclude(bridge);
        }
    }
    for (_, callback) in module.foreign_callback_adapters.iter() {
        exclude(callback.function);
    }
    for (_, coroutine) in module.meta.coroutine_functions.iter() {
        exclude(coroutine.function);
        if let mir::CoroutineLowering::StateMachine { driver, .. } = coroutine.lowering {
            exclude(driver);
        }
    }
    for (_, point) in module.meta.coroutine_resume_points.iter() {
        exclude(point.resume());
        exclude(point.resume_with_exception());
    }
    for start in &module.meta.coroutine_starts {
        exclude(start.function());
    }
    excluded
}

pub(super) fn supported(function: &mir::Function) -> bool {
    executable(function) && supported_blocks(function.body.blocks.iter().map(|(_, b)| b))
}

pub(super) fn executable(function: &mir::Function) -> bool {
    function.body.blocks.iter().any(|(_, block)| {
        !block.statements.is_empty() || !matches!(block.terminator, mir::Terminator::Unreachable)
    })
}

pub(super) fn supported_blocks<'a>(blocks: impl Iterator<Item = &'a mir::BasicBlock>) -> bool {
    let mut supported = true;
    for block in blocks {
        supported &= !matches!(
            block.terminator,
            mir::Terminator::Resume | mir::Terminator::Rethrow { .. }
        );
        supported &= !block
            .statements
            .iter()
            .any(|statement| matches!(statement.kind, mir::StatementKind::Eh(_)));
        mir::visit_block_exprs(block, &mut |expression| {
            supported &= !matches!(expression.kind, mir::ExprKind::CaughtException);
        });
    }
    supported
}

mod cost;
mod heat;
mod score;
pub(super) use cost::{
    Cost, adjusted_blocks_cost, assignment_cost, block_cost, blocks_cost, cold_blocks,
    expression_cost, function_cost, statement_cost, tree_cost,
};
pub(super) use heat::heats;
pub(super) use score::{Growth, Site, assess};

pub(super) fn copy_cost(structs: &la_arena::Arena<mir::StructDef>, ty: &mir::Type) -> usize {
    match ty {
        mir::Type::Struct(id)
            if matches!(
                structs[*id].representation,
                mir::StructRepresentation::Intrinsic(
                    mir::IntrinsicStructRepresentation::Float(_)
                        | mir::IntrinsicStructRepresentation::Char
                )
            ) =>
        {
            0
        }
        mir::Type::Struct(_) | mir::Type::Tuple(_) | mir::Type::Enum(..) => 3,
        mir::Type::Interface(_) => 1,
        _ => 0,
    }
}
