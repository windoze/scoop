use super::super::{
    flow::{ActualType, ValueFacts},
    graph, mir,
};

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
    let mut supported = true;
    let mut executable = false;
    for (_, block) in function.body.blocks.iter() {
        executable |= !block.statements.is_empty()
            || !matches!(block.terminator, mir::Terminator::Unreachable);
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
    supported && executable
}

mod cost;
pub(super) use cost::{Cost, blocks_cost, function_cost};

pub(super) fn worthwhile(
    structs: &la_arena::Arena<mir::StructDef>,
    cost: usize,
    original: usize,
    in_loop: bool,
    arguments: &[ValueFacts],
    function: &mir::Function,
) -> bool {
    if cost <= 12 {
        return true;
    }
    let folds = cost < original && arguments.iter().any(|argument| argument.constant.is_some());
    let receiver = arguments.iter().any(|argument| argument.actual != ActualType::Unknown)
        && function.body.blocks.iter().any(|(_, block)| block.statements.iter().any(|statement| {
            matches!(&statement.kind, mir::StatementKind::Call(effect) if !matches!(super::super::call(effect).target, mir::CallTarget::Direct(_)))
        }));
    let value_copy = function
        .params
        .iter()
        .any(|parameter| copy_cost(structs, &parameter.ty) > 0);
    let threshold = (20
        + usize::from(in_loop) * 16
        + usize::from(folds) * 12
        + usize::from(receiver) * 8
        + usize::from(value_copy) * 4)
        .min(48);
    cost <= threshold
}

fn copy_cost(structs: &la_arena::Arena<mir::StructDef>, ty: &mir::Type) -> usize {
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
