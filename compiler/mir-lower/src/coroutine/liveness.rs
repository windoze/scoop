use super::*;

pub(super) fn analyze_sites(
    lowerer: &Lowerer,
    function: mir::FunctionId,
    body: &mir::Body,
) -> Vec<SuspendSite> {
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
            if let Some((destination, result, kind, pending)) =
                suspend_effect(lowerer, body, statement)
            {
                pending_uses(&pending, &mut live);
                let mut live_after: Vec<_> = live.iter().copied().collect();
                live_after.sort_by_key(|local| raw(*local));
                sites.push(DiscoveredSuspendSite {
                    block: block_id,
                    statement: statement_index,
                    destination,
                    result,
                    kind,
                    live_after,
                    pending,
                });
            }
            let (statement_uses, statement_defs) = statement_use_def(statement);
            for local in statement_defs {
                live.remove(&local);
            }
            live.extend(statement_uses);
        }
    }
    if sites.is_empty() {
        return Vec::new();
    }
    let call_sites = lowerer.coroutines.call_sites(function);
    let call_rank = call_sites
        .iter()
        .enumerate()
        .map(|(rank, site)| ((site.block, site.statement), rank))
        .collect::<HashMap<_, _>>();
    assert_eq!(
        call_rank.len(),
        call_sites.len(),
        "one structured call location occurs once in its semantic order"
    );
    let mut identity_locations = sites
        .iter()
        .map(|site| (site.block, site.statement))
        .collect::<Vec<_>>();
    identity_locations.sort_by_key(|location| {
        *call_rank
            .get(location)
            .expect("every pre-coroutine suspend call has a structured-call position")
    });
    let identity_ordinals = identity_locations
        .into_iter()
        .enumerate()
        .map(|(ordinal, location)| {
            (
                location,
                u32::try_from(ordinal).expect("coroutine suspension-site count fits u32"),
            )
        })
        .collect::<HashMap<_, _>>();

    sites.sort_by_key(|site| (raw(site.block), site.statement));
    sites
        .into_iter()
        .enumerate()
        .map(|(index, site)| {
            let identity_ordinal = identity_ordinals[&(site.block, site.statement)];
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
                pending: site.pending,
                state: mir::CoroutineSuspendStateId::new(
                    u32::try_from(one_based).expect("coroutine suspension-site count fits u32"),
                )
                .expect("coroutine suspension states are one-based"),
                identity_path: hir::StructuralDefinitionPath::from_first(
                    hir::StructuralPathSegment::new(
                        hir::StructuralDefinitionSiteRole::CoroutineTransform,
                        identity_ordinal,
                    ),
                    [],
                ),
            }
        })
        .collect()
}

fn suspend_effect(
    lowerer: &Lowerer,
    body: &mir::Body,
    statement: &mir::Statement,
) -> Option<(
    Option<mir::LocalId>,
    mir::Type,
    SuspendKind,
    mir::CoroutinePendingContext,
)> {
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
        return Some((
            destination,
            result,
            SuspendKind::Intrinsic { register },
            call.pending.clone(),
        ));
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
                    call.pending.clone(),
                )
            });
        }
        mir::Callee::CoroutineSuspend { .. }
        | mir::Callee::Extern(_)
        | mir::Callee::External(_)
        | mir::Callee::Runtime(_) => {
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
                    call.pending.clone(),
                )
            })
        })
}

fn pending_uses(pending: &mir::CoroutinePendingContext, uses: &mut HashSet<mir::LocalId>) {
    let mir::CoroutinePendingContext::Chain(chain) = pending else {
        return;
    };
    for transfer in chain.iter() {
        match transfer {
            mir::CoroutinePendingSourceTransfer::Return(
                mir::CoroutinePendingSourceReturn::Value(value),
            ) => expr_uses(value.expression(), uses),
            mir::CoroutinePendingSourceTransfer::ManagedThrow(throw_) => {
                expr_uses(throw_.exception(), uses);
            }
            mir::CoroutinePendingSourceTransfer::Fallthrough(_)
            | mir::CoroutinePendingSourceTransfer::Return(
                mir::CoroutinePendingSourceReturn::Unit,
            )
            | mir::CoroutinePendingSourceTransfer::Break(_)
            | mir::CoroutinePendingSourceTransfer::Continue(_) => {}
        }
    }
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
        | mir::ExprKind::VariantConstruct {
            fields: elements, ..
        } => {
            for element in elements {
                expr_uses(element, uses);
            }
        }
        mir::ExprKind::ClosureAlloc { captures, .. } => {
            for capture in captures {
                expr_uses(capture.value(), uses);
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
