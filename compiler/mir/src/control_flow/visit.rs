use super::*;

/// Visit an expression tree in pre-order.
pub fn visit_expr(expr: &Expr, visitor: &mut impl FnMut(&Expr)) {
    visitor(expr);
    walk_expr(expr, visitor);
}

/// Visit the immediate children of `expr`, recursively visiting each child's
/// complete subtree. This is separate from [`visit_expr`] so a visitor can
/// choose whether to process the root itself.
pub fn walk_expr(expr: &Expr, visitor: &mut impl FnMut(&Expr)) {
    match &expr.kind {
        ExprKind::Context(operation) => {
            if let Some(operand) = operation.operand() {
                visit_expr(operand, visitor);
            }
        }
        ExprKind::TupleLiteral(values)
        | ExprKind::StructInit { args: values, .. }
        | ExprKind::StructConstruct { fields: values, .. }
        | ExprKind::ArrayLiteral {
            elements: values, ..
        }
        | ExprKind::VariantConstruct { fields: values, .. } => {
            for value in values {
                visit_expr(value, visitor);
            }
        }
        ExprKind::ClosureAlloc { captures, .. } => {
            for capture in captures {
                visit_expr(capture.value(), visitor);
            }
        }
        ExprKind::ArrayAssembly { parts, .. } => {
            for part in parts {
                match part {
                    ArrayAssemblyPart::Element(value) | ArrayAssemblyPart::CopyArray(value) => {
                        visit_expr(value, visitor);
                    }
                }
            }
        }
        ExprKind::ClosureCapture {
            closure: operand, ..
        }
        | ExprKind::PtrFromNonZeroULong { operand, .. }
        | ExprKind::CharCode(operand)
        | ExprKind::CharFromCodeUnchecked(operand)
        | ExprKind::PtrToULong(operand)
        | ExprKind::PtrCast { operand, .. }
        | ExprKind::ForeignCallbackRegister {
            closure: operand, ..
        }
        | ExprKind::ForeignCallbackOperation {
            callback: operand, ..
        }
        | ExprKind::Retype { operand, .. }
        | ExprKind::FieldAccess {
            receiver: operand, ..
        }
        | ExprKind::AtomicFieldLoad {
            object: operand, ..
        }
        | ExprKind::Box(operand)
        | ExprKind::Unbox(operand)
        | ExprKind::IsInstance { operand, .. }
        | ExprKind::Cast { operand, .. }
        | ExprKind::ArrayAllocate { count: operand, .. }
        | ExprKind::ArrayLen { operand, .. }
        | ExprKind::ArrayClone { operand, .. }
        | ExprKind::Unary { operand, .. }
        | ExprKind::IntegerUnary { operand, .. }
        | ExprKind::IntegerConversion { operand, .. }
        | ExprKind::EnumTag(operand)
        | ExprKind::EnumField { operand, .. }
        | ExprKind::VariantTest { operand, .. }
        | ExprKind::VariantPayloadProject { operand, .. } => visit_expr(operand, visitor),
        ExprKind::PtrLoad {
            pointer, offset, ..
        } => {
            visit_expr(pointer, visitor);
            if let Some(offset) = offset {
                visit_expr(offset, visitor);
            }
        }
        ExprKind::PtrStore {
            pointer,
            offset,
            value,
            ..
        } => {
            visit_expr(pointer, visitor);
            if let Some(offset) = offset {
                visit_expr(offset, visitor);
            }
            visit_expr(value, visitor);
        }
        ExprKind::PtrOffset {
            pointer, offset, ..
        } => {
            visit_expr(pointer, visitor);
            visit_expr(offset, visitor);
        }
        ExprKind::AtomicFieldCompareExchange {
            object,
            expected,
            replacement,
            ..
        } => {
            visit_expr(object, visitor);
            visit_expr(expected, visitor);
            visit_expr(replacement, visitor);
        }
        ExprKind::ArrayGet { array, index, .. } => {
            visit_expr(array, visitor);
            visit_expr(index, visitor);
        }
        ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::IntegerBinary { lhs, rhs, .. }
        | ExprKind::SafeIntegerDivRem { lhs, rhs, .. }
        | ExprKind::IntegerCompare { lhs, rhs, .. }
        | ExprKind::IntegerCompareTo { lhs, rhs, .. }
        | ExprKind::IntegerShift {
            value: lhs,
            count: rhs,
            ..
        } => {
            visit_expr(lhs, visitor);
            visit_expr(rhs, visitor);
        }
        ExprKind::StringConst(_)
        | ExprKind::IntegerLiteral(_)
        | ExprKind::MachineScalarLiteral(_)
        | ExprKind::CharLiteral(_)
        | ExprKind::BoolLiteral(_)
        | ExprKind::ReleaseFieldLoad { .. }
        | ExprKind::UnitLiteral
        | ExprKind::ClassAlloc { .. }
        | ExprKind::Local(_)
        | ExprKind::GlobalRead(_)
        | ExprKind::InitializationUnitAddress(_)
        | ExprKind::AddressOf { .. }
        | ExprKind::GlobalAddress { .. }
        | ExprKind::SizeOf(_)
        | ExprKind::AlignOf(_)
        | ExprKind::FunctionAddress { .. }
        | ExprKind::CaughtException => {}
    }
}

/// Mutably visit an expression tree in pre-order.
pub fn visit_expr_mut(expr: &mut Expr, visitor: &mut impl FnMut(&mut Expr)) {
    visitor(expr);
    walk_expr_mut(expr, visitor);
}

/// Mutably visit the immediate children of `expr`, recursively visiting each
/// child's complete subtree.
pub fn walk_expr_mut(expr: &mut Expr, visitor: &mut impl FnMut(&mut Expr)) {
    match &mut expr.kind {
        ExprKind::Context(operation) => {
            if let Some(operand) = operation.operand_mut() {
                visit_expr_mut(operand, visitor);
            }
        }
        ExprKind::TupleLiteral(values)
        | ExprKind::StructInit { args: values, .. }
        | ExprKind::StructConstruct { fields: values, .. }
        | ExprKind::ArrayLiteral {
            elements: values, ..
        }
        | ExprKind::VariantConstruct { fields: values, .. } => {
            for value in values {
                visit_expr_mut(value, visitor);
            }
        }
        ExprKind::ClosureAlloc { captures, .. } => {
            for capture in captures {
                visit_expr_mut(capture.value_mut(), visitor);
            }
        }
        ExprKind::ArrayAssembly { parts, .. } => {
            for part in parts {
                match part {
                    ArrayAssemblyPart::Element(value) | ArrayAssemblyPart::CopyArray(value) => {
                        visit_expr_mut(value, visitor);
                    }
                }
            }
        }
        ExprKind::ClosureCapture {
            closure: operand, ..
        }
        | ExprKind::PtrFromNonZeroULong { operand, .. }
        | ExprKind::CharCode(operand)
        | ExprKind::CharFromCodeUnchecked(operand)
        | ExprKind::PtrToULong(operand)
        | ExprKind::PtrCast { operand, .. }
        | ExprKind::ForeignCallbackRegister {
            closure: operand, ..
        }
        | ExprKind::ForeignCallbackOperation {
            callback: operand, ..
        }
        | ExprKind::Retype { operand, .. }
        | ExprKind::FieldAccess {
            receiver: operand, ..
        }
        | ExprKind::AtomicFieldLoad {
            object: operand, ..
        }
        | ExprKind::Box(operand)
        | ExprKind::Unbox(operand)
        | ExprKind::IsInstance { operand, .. }
        | ExprKind::Cast { operand, .. }
        | ExprKind::ArrayAllocate { count: operand, .. }
        | ExprKind::ArrayLen { operand, .. }
        | ExprKind::ArrayClone { operand, .. }
        | ExprKind::Unary { operand, .. }
        | ExprKind::IntegerUnary { operand, .. }
        | ExprKind::IntegerConversion { operand, .. }
        | ExprKind::EnumTag(operand)
        | ExprKind::EnumField { operand, .. }
        | ExprKind::VariantTest { operand, .. }
        | ExprKind::VariantPayloadProject { operand, .. } => visit_expr_mut(operand, visitor),
        ExprKind::PtrLoad {
            pointer, offset, ..
        } => {
            visit_expr_mut(pointer, visitor);
            if let Some(offset) = offset {
                visit_expr_mut(offset, visitor);
            }
        }
        ExprKind::PtrStore {
            pointer,
            offset,
            value,
            ..
        } => {
            visit_expr_mut(pointer, visitor);
            if let Some(offset) = offset {
                visit_expr_mut(offset, visitor);
            }
            visit_expr_mut(value, visitor);
        }
        ExprKind::PtrOffset {
            pointer, offset, ..
        } => {
            visit_expr_mut(pointer, visitor);
            visit_expr_mut(offset, visitor);
        }
        ExprKind::AtomicFieldCompareExchange {
            object,
            expected,
            replacement,
            ..
        } => {
            visit_expr_mut(object, visitor);
            visit_expr_mut(expected, visitor);
            visit_expr_mut(replacement, visitor);
        }
        ExprKind::ArrayGet { array, index, .. } => {
            visit_expr_mut(array, visitor);
            visit_expr_mut(index, visitor);
        }
        ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::IntegerBinary { lhs, rhs, .. }
        | ExprKind::SafeIntegerDivRem { lhs, rhs, .. }
        | ExprKind::IntegerCompare { lhs, rhs, .. }
        | ExprKind::IntegerCompareTo { lhs, rhs, .. }
        | ExprKind::IntegerShift {
            value: lhs,
            count: rhs,
            ..
        } => {
            visit_expr_mut(lhs, visitor);
            visit_expr_mut(rhs, visitor);
        }
        ExprKind::StringConst(_)
        | ExprKind::IntegerLiteral(_)
        | ExprKind::MachineScalarLiteral(_)
        | ExprKind::CharLiteral(_)
        | ExprKind::BoolLiteral(_)
        | ExprKind::ReleaseFieldLoad { .. }
        | ExprKind::UnitLiteral
        | ExprKind::ClassAlloc { .. }
        | ExprKind::Local(_)
        | ExprKind::GlobalRead(_)
        | ExprKind::InitializationUnitAddress(_)
        | ExprKind::AddressOf { .. }
        | ExprKind::GlobalAddress { .. }
        | ExprKind::SizeOf(_)
        | ExprKind::AlignOf(_)
        | ExprKind::FunctionAddress { .. }
        | ExprKind::CaughtException => {}
    }
}

/// Visit every expression in a block, including call arguments and the terminator.
pub fn visit_block_exprs(block: &BasicBlock, visitor: &mut impl FnMut(&Expr)) {
    for statement in &block.statements {
        match &statement.kind {
            StatementKind::Expr(expr) => visit_expr(expr, visitor),
            StatementKind::Call(effect) => {
                let call = match effect {
                    CallEffect::Unit(call) | CallEffect::Value { call, .. } => call,
                };
                for argument in &call.args {
                    visit_expr(argument, visitor);
                }
            }
            StatementKind::ValDecl { init, .. } => visit_expr(init, visitor),
            StatementKind::PublishReleaseReady { receiver, .. } => visit_expr(receiver, visitor),
            StatementKind::Assign { value, .. } | StatementKind::GlobalAssign { value, .. } => {
                visit_expr(value, visitor)
            }
            StatementKind::ArraySet {
                array,
                index,
                value,
                ..
            } => {
                visit_expr(array, visitor);
                visit_expr(index, visitor);
                visit_expr(value, visitor);
            }
            StatementKind::FieldSet { object, value, .. }
            | StatementKind::AtomicFieldStore { object, value, .. } => {
                visit_expr(object, visitor);
                visit_expr(value, visitor);
            }
            StatementKind::Eh(_) => {}
        }
    }
    match &block.terminator {
        Terminator::Branch { cond, .. } => visit_expr(cond, visitor),
        Terminator::Return { value: Some(value) } => visit_expr(value, visitor),
        Terminator::Throw { exception, .. } => visit_expr(exception, visitor),
        Terminator::Goto(_)
        | Terminator::Return { value: None }
        | Terminator::Rethrow { .. }
        | Terminator::Resume
        | Terminator::Trap { .. }
        | Terminator::Unreachable => {}
    }
}
