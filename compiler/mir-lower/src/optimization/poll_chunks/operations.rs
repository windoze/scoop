//! Operations that cannot introduce a safepoint or an exceptional exit.

use super::mir;

pub(super) fn supported(module: &mir::Module, block: &mir::BasicBlock, arrays: bool) -> bool {
    if block
        .statements
        .iter()
        .any(|statement| match &statement.kind {
            mir::StatementKind::ValDecl { .. }
            | mir::StatementKind::Assign { .. }
            | mir::StatementKind::Expr(_) => false,
            mir::StatementKind::ArraySet { value, .. } => !arrays || !scalar(module, &value.ty),
            _ => true,
        })
    {
        return false;
    }
    let mut supported = true;
    mir::visit_block_exprs(block, &mut |expr| {
        let operation = match &expr.kind {
            mir::ExprKind::ArrayGet { .. } => arrays && scalar(module, &expr.ty),
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
                op: mir::BinOp::BoolEq | mir::BinOp::BoolNe,
                ..
            } => true,
            _ => false,
        };
        let ty = scalar(module, &expr.ty)
            || expr.ty == mir::Type::Unit
            || scalar_array(module, &expr.ty);
        supported &= operation && ty;
    });
    supported
}

fn scalar(module: &mir::Module, ty: &mir::Type) -> bool {
    match ty {
        mir::Type::Integer(_) | mir::Type::Boolean => true,
        mir::Type::Struct(id) => matches!(
            module.structs[*id].representation,
            mir::StructRepresentation::Intrinsic(mir::IntrinsicStructRepresentation::Float(_))
        ),
        _ => false,
    }
}

fn scalar_array(module: &mir::Module, ty: &mir::Type) -> bool {
    let mir::Type::Class(class) = ty else {
        return false;
    };
    match &module.classes[*class].representation {
        mir::ClassRepresentation::Intrinsic(
            mir::IntrinsicClassRepresentation::Array { element }
            | mir::IntrinsicClassRepresentation::MutableArray { element },
        ) => scalar(module, element),
        _ => false,
    }
}
