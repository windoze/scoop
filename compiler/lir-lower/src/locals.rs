use std::collections::HashSet;

use scoop_mir as mir;

/// Find MIR locals whose stable address is observed by a raw-pointer operation.
///
/// Parameters normally stay as SSA values in LIR. This analysis is deliberately
/// separate from instruction lowering so address-taken parameters can be given a
/// stack slot before the function body is emitted.
pub(super) fn address_taken(function: &mir::Function) -> HashSet<mir::LocalId> {
    fn collect_expr(value: &mir::Expr, out: &mut HashSet<mir::LocalId>) {
        match &value.kind {
            mir::ExprKind::AddressOf { local, .. } => {
                out.insert(*local);
            }
            mir::ExprKind::TupleLiteral(values)
            | mir::ExprKind::ArrayLiteral {
                elements: values, ..
            }
            | mir::ExprKind::StructInit { args: values, .. }
            | mir::ExprKind::StructConstruct { fields: values, .. }
            | mir::ExprKind::ClosureAlloc {
                captures: values, ..
            }
            | mir::ExprKind::VariantConstruct { fields: values, .. } => {
                for value in values {
                    collect_expr(value, out);
                }
            }
            mir::ExprKind::ArrayAssembly { parts, .. } => {
                for part in parts {
                    match part {
                        mir::ArrayAssemblyPart::Element(value)
                        | mir::ArrayAssemblyPart::CopyArray(value) => collect_expr(value, out),
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
            | mir::ExprKind::IsInstance { operand, .. }
            | mir::ExprKind::Cast { operand, .. }
            | mir::ExprKind::ArrayLen { operand, .. }
            | mir::ExprKind::ArrayClone { operand, .. }
            | mir::ExprKind::Unary { operand, .. }
            | mir::ExprKind::IntegerUnary { operand, .. }
            | mir::ExprKind::IntegerConversion { operand, .. }
            | mir::ExprKind::EnumTag(operand)
            | mir::ExprKind::EnumField { operand, .. }
            | mir::ExprKind::VariantTest { operand, .. }
            | mir::ExprKind::VariantPayloadProject { operand, .. }
            | mir::ExprKind::PtrFromNonZeroULong { operand, .. }
            | mir::ExprKind::PtrToULong(operand)
            | mir::ExprKind::PtrCast { operand, .. } => collect_expr(operand, out),
            mir::ExprKind::ClassAlloc { .. } => {}
            mir::ExprKind::AtomicFieldCompareExchange {
                object,
                expected,
                replacement,
                ..
            } => {
                collect_expr(object, out);
                collect_expr(expected, out);
                collect_expr(replacement, out);
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
            }
            | mir::ExprKind::PtrOffset {
                pointer: array,
                offset: index,
                ..
            } => {
                collect_expr(array, out);
                collect_expr(index, out);
            }
            mir::ExprKind::PtrLoad {
                pointer, offset, ..
            } => {
                collect_expr(pointer, out);
                if let Some(offset) = offset {
                    collect_expr(offset, out);
                }
            }
            mir::ExprKind::PtrStore {
                pointer,
                offset,
                value,
                ..
            } => {
                collect_expr(pointer, out);
                if let Some(offset) = offset {
                    collect_expr(offset, out);
                }
                collect_expr(value, out);
            }
            mir::ExprKind::StringConst(_)
            | mir::ExprKind::IntegerLiteral(_)
            | mir::ExprKind::MachineScalarLiteral(_)
            | mir::ExprKind::BoolLiteral(_)
            | mir::ExprKind::UnitLiteral
            | mir::ExprKind::Local(_)
            | mir::ExprKind::GlobalRead(_)
            | mir::ExprKind::InitializationUnitAddress(_)
            | mir::ExprKind::GlobalAddress { .. }
            | mir::ExprKind::CaughtException
            | mir::ExprKind::SizeOf(_)
            | mir::ExprKind::AlignOf(_)
            | mir::ExprKind::FunctionAddress { .. } => {}
        }
    }

    fn collect_call(call: &mir::Call, out: &mut HashSet<mir::LocalId>) {
        for arg in &call.args {
            collect_expr(arg, out);
        }
    }

    let mut out = HashSet::new();
    for (_, block) in function.body.blocks.iter() {
        for statement in &block.statements {
            match &statement.kind {
                mir::StatementKind::Expr(value) => collect_expr(value, &mut out),
                mir::StatementKind::Call(effect) => match effect {
                    mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => {
                        collect_call(call, &mut out)
                    }
                },
                mir::StatementKind::ValDecl { init, .. } => collect_expr(init, &mut out),
                mir::StatementKind::Assign { value, .. } => collect_expr(value, &mut out),
                mir::StatementKind::GlobalAssign { value, .. } => collect_expr(value, &mut out),
                mir::StatementKind::ArraySet {
                    array,
                    index,
                    value,
                    ..
                } => {
                    collect_expr(array, &mut out);
                    collect_expr(index, &mut out);
                    collect_expr(value, &mut out);
                }
                mir::StatementKind::FieldSet { object, value, .. }
                | mir::StatementKind::AtomicFieldStore { object, value, .. } => {
                    collect_expr(object, &mut out);
                    collect_expr(value, &mut out);
                }
                mir::StatementKind::Eh(_) => {}
            }
        }
        match &block.terminator {
            mir::Terminator::Branch { cond, .. } => collect_expr(cond, &mut out),
            mir::Terminator::Return { value } => {
                if let Some(value) = value {
                    collect_expr(value, &mut out);
                }
            }
            mir::Terminator::Throw { exception, .. } => collect_expr(exception, &mut out),
            mir::Terminator::Goto(_)
            | mir::Terminator::Rethrow { .. }
            | mir::Terminator::Resume
            | mir::Terminator::Trap { .. }
            | mir::Terminator::Unreachable => {}
        }
    }
    out
}
