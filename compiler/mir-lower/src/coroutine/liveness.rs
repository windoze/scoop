use super::*;

pub(super) fn analyze_sites(lowerer: &Lowerer, body: &mir::Body) -> Vec<SuspendSite> {
    let block_count = body.blocks.len();
    let mut uses = vec![HashSet::new(); block_count];
    let mut defs = vec![HashSet::new(); block_count];
    for (block_id, block) in body.blocks.iter() {
        let index = raw(block_id);
        for statement in &block.statements {
            let (statement_uses, statement_defs) = statement_use_def(statement);
            for local in statement_uses {
                if !defs[index].contains(&local) {
                    uses[index].insert(local);
                }
            }
            defs[index].extend(statement_defs);
        }
        for local in terminator_uses(&block.terminator) {
            if !defs[index].contains(&local) {
                uses[index].insert(local);
            }
        }
    }

    let mut live_in = vec![HashSet::new(); block_count];
    let mut live_out = vec![HashSet::new(); block_count];
    loop {
        let mut changed = false;
        for (block_id, block) in body.blocks.iter().rev() {
            let index = raw(block_id);
            let mut out = HashSet::new();
            for successor in successors(block) {
                out.extend(live_in[raw(successor)].iter().copied());
            }
            let mut input = uses[index].clone();
            input.extend(out.iter().filter(|local| !defs[index].contains(local)));
            if out != live_out[index] || input != live_in[index] {
                live_out[index] = out;
                live_in[index] = input;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut sites = Vec::new();
    for (block_id, block) in body.blocks.iter() {
        let mut live = live_out[raw(block_id)].clone();
        live.extend(terminator_uses(&block.terminator));
        for (statement_index, statement) in block.statements.iter().enumerate().rev() {
            if let Some((destination, result, kind)) = suspend_effect(lowerer, body, statement) {
                let mut live_after: Vec<_> = live.iter().copied().collect();
                live_after.sort_by_key(|local| raw(*local));
                sites.push(DiscoveredSuspendSite {
                    block: block_id,
                    statement: statement_index,
                    destination,
                    result,
                    kind,
                    live_after,
                });
            }
            let (statement_uses, statement_defs) = statement_use_def(statement);
            for local in statement_defs {
                live.remove(&local);
            }
            live.extend(statement_uses);
        }
    }
    sites.sort_by_key(|site| (raw(site.block), site.statement));
    sites
        .into_iter()
        .enumerate()
        .map(|(index, site)| {
            let one_based = index
                .checked_add(1)
                .expect("coroutine suspension-site count fits usize");
            SuspendSite {
                block: site.block,
                statement: site.statement,
                destination: site.destination,
                result: site.result,
                kind: site.kind,
                live_after: site.live_after,
                state: mir::CoroutineSuspendStateId::new(
                    u32::try_from(one_based).expect("coroutine suspension-site count fits u32"),
                )
                .expect("coroutine suspension states are one-based"),
            }
        })
        .collect()
}

fn suspend_effect(
    lowerer: &Lowerer,
    body: &mir::Body,
    statement: &mir::Statement,
) -> Option<(Option<mir::LocalId>, mir::Type, SuspendKind)> {
    let mir::StatementKind::Call(effect) = &statement.kind else {
        return None;
    };
    let (call, destination) = match effect {
        mir::CallEffect::Unit(call) => (call, None),
        mir::CallEffect::Value { destination, call } => (call, Some(*destination)),
    };
    if let mir::Callee::CoroutineSuspend { register } = call.target.callee {
        let result = destination
            .map(|local| body.locals[local].ty.clone())
            .unwrap_or(mir::Type::Unit);
        return Some((destination, result, SuspendKind::Intrinsic { register }));
    }
    let function = match call.target.callee {
        mir::Callee::User(function) => function,
        mir::Callee::Monomorphized(instance) => lowerer.instances.meta[instance].function,
        mir::Callee::Closure(function_type) | mir::Callee::FunctionBridge(function_type) => {
            let signature = &lowerer.shell.function_types[function_type];
            return signature.is_suspend.then(|| {
                (
                    destination,
                    signature.return_type.clone(),
                    SuspendKind::Call,
                )
            });
        }
        mir::Callee::CoroutineSuspend { .. } | mir::Callee::Extern(_) | mir::Callee::Runtime(_) => {
            return None;
        }
    };
    lowerer
        .coroutines
        .functions
        .iter()
        .find_map(|(_, coroutine)| {
            (coroutine.function == function).then(|| {
                (
                    destination,
                    coroutine.source_return.clone(),
                    SuspendKind::Call,
                )
            })
        })
}

fn statement_use_def(statement: &mir::Statement) -> (HashSet<mir::LocalId>, HashSet<mir::LocalId>) {
    let mut uses = HashSet::new();
    let mut defs = HashSet::new();
    match &statement.kind {
        mir::StatementKind::Expr(expr) => expr_uses(expr, &mut uses),
        mir::StatementKind::Call(effect) => match effect {
            mir::CallEffect::Unit(call) => call_uses(call, &mut uses),
            mir::CallEffect::Value { destination, call } => {
                call_uses(call, &mut uses);
                defs.insert(*destination);
            }
        },
        mir::StatementKind::ValDecl { local, init } => {
            expr_uses(init, &mut uses);
            defs.insert(*local);
        }
        mir::StatementKind::Assign { local, value } => {
            expr_uses(value, &mut uses);
            defs.insert(*local);
        }
        mir::StatementKind::GlobalAssign { value, .. } => {
            expr_uses(value, &mut uses);
        }
        mir::StatementKind::ArraySet {
            array,
            index,
            value,
            ..
        } => {
            expr_uses(array, &mut uses);
            expr_uses(index, &mut uses);
            expr_uses(value, &mut uses);
        }
        mir::StatementKind::FieldSet { object, value, .. }
        | mir::StatementKind::AtomicFieldStore { object, value, .. } => {
            expr_uses(object, &mut uses);
            expr_uses(value, &mut uses);
        }
        mir::StatementKind::Eh(_) => {}
    }
    (uses, defs)
}

fn terminator_uses(terminator: &mir::Terminator) -> HashSet<mir::LocalId> {
    let mut uses = HashSet::new();
    match terminator {
        mir::Terminator::Branch { cond, .. } => expr_uses(cond, &mut uses),
        mir::Terminator::Return { value } => {
            if let Some(value) = value {
                expr_uses(value, &mut uses);
            }
        }
        mir::Terminator::Throw { exception, .. } => expr_uses(exception, &mut uses),
        mir::Terminator::Goto(_)
        | mir::Terminator::Rethrow { .. }
        | mir::Terminator::Resume
        | mir::Terminator::Trap { .. }
        | mir::Terminator::Unreachable => {}
    }
    uses
}

fn successors(block: &mir::BasicBlock) -> Vec<mir::BlockId> {
    let mut out = Vec::new();
    match block.terminator {
        mir::Terminator::Goto(target) => out.push(target),
        mir::Terminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            out.push(then_block);
            out.push(else_block);
        }
        mir::Terminator::Throw { unwind, .. } | mir::Terminator::Rethrow { unwind } => {
            if let Some(unwind) = unwind {
                out.push(unwind);
            }
        }
        mir::Terminator::Return { .. }
        | mir::Terminator::Resume
        | mir::Terminator::Trap { .. }
        | mir::Terminator::Unreachable => {}
    }
    if let Some(unwind) = block.unwind
        && !out.contains(&unwind)
    {
        out.push(unwind);
    }
    out
}

fn call_uses(call: &mir::Call, uses: &mut HashSet<mir::LocalId>) {
    for arg in &call.args {
        expr_uses(arg, uses);
    }
}

fn expr_uses(expr: &mir::Expr, uses: &mut HashSet<mir::LocalId>) {
    match &expr.kind {
        mir::ExprKind::Local(local) => {
            uses.insert(*local);
        }
        mir::ExprKind::TupleLiteral(elements)
        | mir::ExprKind::ArrayLiteral { elements, .. }
        | mir::ExprKind::StructInit { args: elements, .. }
        | mir::ExprKind::StructConstruct {
            fields: elements, ..
        }
        | mir::ExprKind::ClosureAlloc {
            captures: elements, ..
        }
        | mir::ExprKind::VariantConstruct {
            fields: elements, ..
        } => {
            for element in elements {
                expr_uses(element, uses);
            }
        }
        mir::ExprKind::ArrayAssembly { parts, .. } => {
            for part in parts {
                match part {
                    mir::ArrayAssemblyPart::Element(value)
                    | mir::ArrayAssemblyPart::CopyArray(value) => expr_uses(value, uses),
                }
            }
        }
        mir::ExprKind::Retype { operand, .. }
        | mir::ExprKind::ClosureCapture {
            closure: operand, ..
        }
        | mir::ExprKind::ForeignCallbackRegister {
            closure: operand, ..
        }
        | mir::ExprKind::ForeignCallbackOperation {
            callback: operand, ..
        }
        | mir::ExprKind::FieldAccess {
            receiver: operand, ..
        }
        | mir::ExprKind::AtomicFieldLoad {
            object: operand, ..
        }
        | mir::ExprKind::Box(operand)
        | mir::ExprKind::Unbox(operand)
        | mir::ExprKind::IsInstance {
            operand,
            check_ty: _,
        }
        | mir::ExprKind::Cast { operand, .. }
        | mir::ExprKind::ArrayLen { operand, .. }
        | mir::ExprKind::ArrayClone { operand, .. }
        | mir::ExprKind::PtrFromNonZeroULong { operand, .. }
        | mir::ExprKind::PtrToULong(operand)
        | mir::ExprKind::PtrCast { operand, .. }
        | mir::ExprKind::Unary { operand, .. }
        | mir::ExprKind::IntegerUnary { operand, .. }
        | mir::ExprKind::IntegerConversion { operand, .. }
        | mir::ExprKind::EnumTag(operand)
        | mir::ExprKind::EnumField { operand, .. }
        | mir::ExprKind::VariantTest { operand, .. }
        | mir::ExprKind::VariantPayloadProject { operand, .. } => expr_uses(operand, uses),
        mir::ExprKind::AtomicFieldCompareExchange {
            object,
            expected,
            replacement,
            ..
        } => {
            expr_uses(object, uses);
            expr_uses(expected, uses);
            expr_uses(replacement, uses);
        }
        mir::ExprKind::ArrayGet { array, index, .. }
        | mir::ExprKind::Binary {
            lhs: array,
            rhs: index,
            ..
        }
        | mir::ExprKind::IntegerBinary {
            lhs: array,
            rhs: index,
            ..
        }
        | mir::ExprKind::SafeIntegerDivRem {
            lhs: array,
            rhs: index,
            ..
        }
        | mir::ExprKind::IntegerCompare {
            lhs: array,
            rhs: index,
            ..
        }
        | mir::ExprKind::IntegerCompareTo {
            lhs: array,
            rhs: index,
            ..
        }
        | mir::ExprKind::IntegerShift {
            value: array,
            count: index,
            ..
        } => {
            expr_uses(array, uses);
            expr_uses(index, uses);
        }
        mir::ExprKind::PtrLoad {
            pointer, offset, ..
        } => {
            expr_uses(pointer, uses);
            if let Some(offset) = offset {
                expr_uses(offset, uses);
            }
        }
        mir::ExprKind::PtrStore {
            pointer,
            offset,
            value,
            ..
        } => {
            expr_uses(pointer, uses);
            if let Some(offset) = offset {
                expr_uses(offset, uses);
            }
            expr_uses(value, uses);
        }
        mir::ExprKind::PtrOffset {
            pointer, offset, ..
        } => {
            expr_uses(pointer, uses);
            expr_uses(offset, uses);
        }
        mir::ExprKind::AddressOf { local, .. } => {
            uses.insert(*local);
        }
        mir::ExprKind::ClassAlloc { .. }
        | mir::ExprKind::StringConst(_)
        | mir::ExprKind::IntegerLiteral(_)
        | mir::ExprKind::MachineScalarLiteral(_)
        | mir::ExprKind::BoolLiteral(_)
        | mir::ExprKind::UnitLiteral
        | mir::ExprKind::GlobalRead(_)
        | mir::ExprKind::InitializationUnitAddress(_)
        | mir::ExprKind::GlobalAddress { .. }
        | mir::ExprKind::CaughtException
        | mir::ExprKind::SizeOf(_)
        | mir::ExprKind::AlignOf(_)
        | mir::ExprKind::FunctionAddress { .. } => {}
    }
}

pub(super) fn raw<T>(id: la_arena::Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
