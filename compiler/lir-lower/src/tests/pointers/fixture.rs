use super::*;

pub(super) fn pointer(pointee: &mir::Type, bits: u64) -> mir::Expr {
    pointer_from(pointee, ulong_expr(bits))
}

pub(super) fn pointer_from(pointee: &mir::Type, bits: mir::Expr) -> mir::Expr {
    expr(
        mir::Type::Ptr(Box::new(pointee.clone())),
        mir::ExprKind::PtrFromNonZeroULong {
            operand: Box::new(bits),
            pointee: Box::new(pointee.clone()),
        },
    )
}

pub(super) fn load(pointer: mir::Expr, offset: Option<mir::Expr>) -> mir::Expr {
    let mir::Type::Ptr(pointee) = pointer.ty.clone() else {
        panic!("pointer fixture requires a raw pointer");
    };
    expr(
        *pointee.clone(),
        mir::ExprKind::PtrLoad {
            pointer: Box::new(pointer),
            pointee,
            offset: offset.map(Box::new),
        },
    )
}

pub(super) fn store(pointer: mir::Expr, offset: Option<mir::Expr>, value: mir::Expr) -> mir::Expr {
    expr(
        mir::Type::Unit,
        mir::ExprKind::PtrStore {
            pointer: Box::new(pointer),
            pointee: Box::new(value.ty.clone()),
            offset: offset.map(Box::new),
            value: Box::new(value),
        },
    )
}

pub(super) fn offset(pointer: mir::Expr, offset: mir::Expr, subtract: bool) -> mir::Expr {
    let mir::Type::Ptr(pointee) = pointer.ty.clone() else {
        panic!("pointer fixture requires a raw pointer");
    };
    expr(
        pointer.ty.clone(),
        mir::ExprKind::PtrOffset {
            pointer: Box::new(pointer),
            pointee,
            offset: Box::new(offset),
            subtract,
        },
    )
}

pub(super) fn source(mut builder: Builder, expressions: Vec<mir::Expr>) -> mir::Module {
    let main = builder.main(
        Arena::new(),
        expressions.into_iter().map(expr_stmt).collect(),
    );
    builder.functions[main].gc_effect = mir::GcEffect::NoGc;
    builder.finish(main)
}

pub(super) fn body_dump(module: &lir::Module) -> String {
    let dump = lir::dump(module);
    dump.lines()
        .skip_while(|line| !line.starts_with("  fun @"))
        .skip(1)
        .take_while(|line| line.starts_with("  block ") || line.starts_with("    "))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn empty_value(id: mir::StructId) -> mir::Expr {
    expr(
        mir::Type::Struct(id),
        mir::ExprKind::StructConstruct {
            struct_id: id,
            fields: Vec::new(),
        },
    )
}
