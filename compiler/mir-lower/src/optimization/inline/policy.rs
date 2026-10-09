use super::super::{
    flow::{ActualType, ValueFacts},
    graph, mir,
};

pub(super) fn local_target(module: &mir::Module, call: &mir::Call) -> Option<mir::FunctionId> {
    if call.target.kind != mir::CallKind::Direct {
        return None;
    }
    match call.target.callee {
        mir::Callee::User(function) => Some(function),
        mir::Callee::Monomorphized(instance) => Some(module.meta.instances[instance].function),
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

pub(super) fn function_cost(function: &mir::Function) -> usize {
    blocks_cost(function.body.blocks.iter().map(|(_, block)| block)) + parameter_cost(function)
}

pub(super) fn blocks_cost<'a>(blocks: impl Iterator<Item = &'a mir::BasicBlock>) -> usize {
    blocks
        .map(|block| {
            let mut cost = usize::from(block.unwind.is_some()) * 4;
            for statement in &block.statements {
                cost += match &statement.kind {
                    mir::StatementKind::Call(effect) => {
                        if super::super::call(effect).target.kind == mir::CallKind::Direct {
                            6
                        } else {
                            10
                        }
                    }
                    mir::StatementKind::ValDecl { init, .. }
                    | mir::StatementKind::Assign { value: init, .. } => 1 + copy_cost(&init.ty),
                    mir::StatementKind::Eh(_) => 8,
                    _ => 2,
                };
            }
            mir::visit_block_exprs(block, &mut |expression| {
                cost += match expression.kind {
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
                };
            });
            cost + match block.terminator {
                mir::Terminator::Goto(_) | mir::Terminator::Unreachable => 0,
                mir::Terminator::Branch { .. } => 2,
                mir::Terminator::Throw { .. }
                | mir::Terminator::Rethrow { .. }
                | mir::Terminator::Resume => 8,
                _ => 1,
            }
        })
        .sum()
}

pub(super) fn parameter_cost(function: &mir::Function) -> usize {
    function
        .params
        .iter()
        .map(|parameter| 1 + copy_cost(&parameter.ty))
        .sum()
}

pub(super) fn worthwhile(
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
            matches!(&statement.kind, mir::StatementKind::Call(effect) if super::super::call(effect).target.kind != mir::CallKind::Direct)
        }));
    let value_copy = function
        .params
        .iter()
        .any(|parameter| copy_cost(&parameter.ty) > 0);
    let threshold = (20
        + usize::from(in_loop) * 16
        + usize::from(folds) * 12
        + usize::from(receiver) * 8
        + usize::from(value_copy) * 4)
        .min(48);
    cost <= threshold
}

fn copy_cost(ty: &mir::Type) -> usize {
    match ty {
        mir::Type::Struct(_) | mir::Type::Tuple(_) | mir::Type::Enum(..) => 3,
        mir::Type::Interface(_) => 1,
        _ => 0,
    }
}
