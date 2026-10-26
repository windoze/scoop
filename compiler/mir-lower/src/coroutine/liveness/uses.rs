use super::*;

pub(super) fn pending_uses(pending: &crate::cfg::CleanupContext, uses: &mut HashSet<mir::LocalId>) {
    use crate::cfg::CleanupTransfer;
    for transfer in &pending.0 {
        match transfer {
            CleanupTransfer::ReturnValue(local)
            | CleanupTransfer::ManagedThrow {
                exception: local, ..
            } => {
                uses.insert(*local);
            }
            CleanupTransfer::Fallthrough(_)
            | CleanupTransfer::ReturnUnit
            | CleanupTransfer::Break(_)
            | CleanupTransfer::Continue(_) => {}
        }
    }
}

pub(super) fn statement_use_def(
    statement: &mir::Statement,
) -> (HashSet<mir::LocalId>, HashSet<mir::LocalId>) {
    let mut uses = HashSet::new();
    let mut defs = HashSet::new();
    match &statement.kind {
        mir::StatementKind::Expr(expr) => expr_uses(expr, &mut uses),
        mir::StatementKind::PublishReleaseReady { receiver, .. } => expr_uses(receiver, &mut uses),
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

pub(super) fn terminator_uses(terminator: &mir::Terminator) -> HashSet<mir::LocalId> {
    let mut uses = HashSet::new();
    match terminator {
        mir::Terminator::Branch { cond, .. } => expr_uses(cond, &mut uses),
        mir::Terminator::Return { value } => {
            expr_uses(value, &mut uses);
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

pub(super) fn terminator_successors(block: &mir::BasicBlock) -> Vec<mir::BlockId> {
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
    out
}

fn call_uses(call: &mir::Call, uses: &mut HashSet<mir::LocalId>) {
    for arg in call.operands() {
        expr_uses(arg, uses);
    }
}

pub(super) fn expr_uses(expr: &mir::Expr, uses: &mut HashSet<mir::LocalId>) {
    match &expr.kind {
        mir::ExprKind::StringByte(operation) => {
            for value in operation.operands() {
                expr_uses(value, uses);
            }
        }
        mir::ExprKind::StringCopy(operation) => {
            for value in operation.operands() {
                expr_uses(value, uses);
            }
        }
        mir::ExprKind::StringConcat(parts) => {
            for value in parts.iter().filter_map(mir::StringConcatPart::value) {
                expr_uses(value, uses);
            }
        }
        mir::ExprKind::MaybeUninit { operation, .. } => {
            if let Some(operand) = operation.operand() {
                expr_uses(operand, uses);
            }
        }
        mir::ExprKind::Atomic(atomic) => {
            for operand in atomic.operands() {
                expr_uses(operand, uses);
            }
        }
        mir::ExprKind::DataBorrow(operation) => expr_uses(&operation.operand, uses),
        mir::ExprKind::Context(operation) => {
            if let Some(operand) = operation.operand() {
                expr_uses(operand, uses);
            }
        }
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
        | mir::ExprKind::ArrayAllocate { count: operand, .. }
        | mir::ExprKind::ArrayLen { operand, .. }
        | mir::ExprKind::ArrayClone { operand, .. }
        | mir::ExprKind::AtomicNew(operand)
        | mir::ExprKind::PtrFromNonZeroULong { operand, .. }
        | mir::ExprKind::CharCode(operand)
        | mir::ExprKind::CharFromCodeUnchecked(operand)
        | mir::ExprKind::PtrToULong(operand)
        | mir::ExprKind::PtrCast { operand, .. }
        | mir::ExprKind::Unary { operand, .. }
        | mir::ExprKind::IntegerUnary { operand, .. }
        | mir::ExprKind::IntegerConversion { operand, .. }
        | mir::ExprKind::FloatUnary { operand, .. }
        | mir::ExprKind::FloatConversion { operand, .. }
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
        | mir::ExprKind::FloatBinary {
            lhs: array,
            rhs: index,
            ..
        }
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
        | mir::ExprKind::CharLiteral(_)
        | mir::ExprKind::FloatLiteral(_)
        | mir::ExprKind::BoolLiteral(_)
        | mir::ExprKind::UnitLiteral
        | mir::ExprKind::ReleaseFieldLoad { .. }
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
