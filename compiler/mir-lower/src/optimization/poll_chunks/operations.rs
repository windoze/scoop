//! Fixed-representation operations without a safepoint or an exceptional exit.

use std::collections::BTreeSet;

use super::mir;

pub(super) fn supported(module: &mir::Module, block: &mir::BasicBlock, arrays: bool) -> bool {
    if block
        .statements
        .iter()
        .any(|statement| match &statement.kind {
            mir::StatementKind::ValDecl { .. }
            | mir::StatementKind::Assign { .. }
            | mir::StatementKind::Expr(_) => false,
            mir::StatementKind::ArraySet { value, .. } => !arrays || !value_type(module, &value.ty),
            _ => true,
        })
    {
        return false;
    }
    let mut supported = true;
    mir::visit_block_exprs(block, &mut |expr| {
        let operation = match &expr.kind {
            mir::ExprKind::ArrayGet { .. }
            | mir::ExprKind::FieldAccess { .. }
            | mir::ExprKind::ClosureCapture { .. }
            | mir::ExprKind::MaybeUninit { .. }
            | mir::ExprKind::VariantConstruct { .. }
            | mir::ExprKind::VariantTest { .. }
            | mir::ExprKind::VariantPayloadProject { .. }
            | mir::ExprKind::StringConst(_) => arrays,
            mir::ExprKind::Retype { operand, .. } => {
                arrays && reference_conversion(module, &operand.ty, &expr.ty)
            }
            mir::ExprKind::ArrayLen { .. } => true,
            mir::ExprKind::IntegerLiteral(_)
            | mir::ExprKind::BoolLiteral(_)
            | mir::ExprKind::UnitLiteral
            | mir::ExprKind::Local(_)
            | mir::ExprKind::IntegerUnary { .. }
            | mir::ExprKind::IntegerBinary { .. }
            | mir::ExprKind::IntegerCompare { .. }
            | mir::ExprKind::IntegerCompareTo { .. }
            | mir::ExprKind::IntegerShift { .. }
            | mir::ExprKind::IntegerConversion { .. }
            | mir::ExprKind::FloatLiteral(_)
            | mir::ExprKind::FloatUnary { .. }
            | mir::ExprKind::FloatBinary { .. }
            | mir::ExprKind::FloatConversion { .. }
            | mir::ExprKind::Unary { .. }
            | mir::ExprKind::Binary {
                op: mir::BinOp::BoolEq | mir::BinOp::BoolNe | mir::BinOp::RefEq | mir::BinOp::RefNe,
                ..
            } => true,
            _ => false,
        };
        let ty = value_type(module, &expr.ty) || expr.ty == mir::Type::Unit;
        supported &= operation && ty;
    });
    supported
}

fn value_type(module: &mir::Module, ty: &mir::Type) -> bool {
    if reference_type(ty) {
        return true;
    }
    match ty {
        mir::Type::Integer(_) | mir::Type::Boolean => true,
        mir::Type::Struct(id) => match &module.structs[*id].representation {
            mir::StructRepresentation::Intrinsic(mir::IntrinsicStructRepresentation::Float(_)) => {
                true
            }
            mir::StructRepresentation::Intrinsic(
                mir::IntrinsicStructRepresentation::MaybeUninit { value },
            ) => value_type(module, value),
            _ => false,
        },
        mir::Type::Enum(id, _) => {
            let definition = &module.enums[*id];
            let [first, second] = definition.variants.as_slice() else {
                return false;
            };
            let fields = if first.fields.is_empty() {
                &second.fields
            } else if second.fields.is_empty() {
                &first.fields
            } else {
                return false;
            };
            matches!(fields.as_slice(), [field] if reference_type(&field.ty))
        }
        _ => false,
    }
}

fn reference_type(ty: &mir::Type) -> bool {
    matches!(
        ty,
        mir::Type::Class(_)
            | mir::Type::Interface(_)
            | mir::Type::Any
            | mir::Type::String
            | mir::Type::Function(_)
    )
}

/// Only conversions that lower to ordinary representation/table loads. The
/// exact parent graph supplies reachability, while LIR still owns table layout.
fn reference_conversion(module: &mir::Module, source: &mir::Type, target: &mir::Type) -> bool {
    if !reference_type(source) || !reference_type(target) {
        return false;
    }
    let mir::Type::Interface(parent) = target else {
        return true;
    };
    if source == target || module.interfaces[*parent].methods.is_empty() {
        return true;
    }
    match source {
        mir::Type::Class(class) => module.classes[*class].modifier == mir::ClassModifier::Final,
        mir::Type::Interface(child) => {
            let mut seen = BTreeSet::new();
            let mut pending = vec![*child];
            while let Some(interface) = pending.pop() {
                if interface == *parent {
                    return true;
                }
                if seen.insert(interface) {
                    pending.extend(&module.interfaces[interface].parents);
                }
            }
            false
        }
        _ => false,
    }
}
