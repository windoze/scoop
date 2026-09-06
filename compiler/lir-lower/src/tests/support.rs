use super::*;
use scoop_ast::Span;

mod builder;

pub(super) use builder::Builder;

pub(super) const SPAN: Span = Span { start: 0, end: 0 };
pub(super) const INT: mir::Type = mir::Type::Integer(mir::IntegerKind::SIGNED_32);
pub(super) const LONG: mir::Type = mir::Type::Integer(mir::IntegerKind::SIGNED_64);
pub(super) const UINT: mir::Type = mir::Type::Integer(mir::IntegerKind::UNSIGNED_32);
pub(super) const ULONG: mir::Type = mir::Type::Integer(mir::IntegerKind::UNSIGNED_64);

pub(super) fn int_expr(value: i32) -> mir::Expr {
    mir::Expr::integer(mir::MirIntegerConstant::Signed32(value as u32))
}

pub(super) fn long_expr(value: i64) -> mir::Expr {
    mir::Expr::integer(mir::MirIntegerConstant::Signed64(value as u64))
}

pub(super) fn uint_expr(value: u32) -> mir::Expr {
    mir::Expr::integer(mir::MirIntegerConstant::Unsigned32(value))
}

pub(super) fn ulong_expr(value: u64) -> mir::Expr {
    mir::Expr::integer(mir::MirIntegerConstant::Unsigned64(value))
}

pub(super) fn integer_expr(kind: mir::IntegerKind, raw_bits: u64) -> mir::Expr {
    mir::Expr::integer(
        mir::MirIntegerConstant::from_raw_bits(kind, raw_bits)
            .expect("test integer bits fit the requested exact width"),
    )
}

/// The reference-field offsets of a plain (non-enum) layout.
pub(super) fn plain_refs(layout: &lir::Layout) -> &[u64] {
    let lir::LayoutKind::Plain { scan } = &layout.kind else {
        panic!("expected a plain layout")
    };
    match scan {
        lir::RefScan::None => &[],
        lir::RefScan::References(offsets) => offsets,
        other => panic!("expected a flat plain scan, found {other:?}"),
    }
}

pub(super) fn layout_values(module: &lir::Module) -> impl Iterator<Item = &lir::Layout> {
    module.meta.layouts.iter().map(|(_, layout)| layout)
}

pub(super) fn descriptor_values(
    module: &lir::Module,
) -> impl Iterator<Item = &lir::TypeDescriptor> {
    module
        .meta
        .type_descriptors
        .iter()
        .map(|(_, descriptor)| descriptor)
}

pub(super) fn descriptor(
    module: &lir::Module,
    reference: lir::TypeDescriptorRef,
) -> &lir::TypeDescriptor {
    let lir::TypeDescriptorRef::Local(id) = reference else {
        panic!("tests expect a local descriptor")
    };
    &module.meta.type_descriptors[id]
}

pub(super) fn fixed_scan(descriptor: &lir::TypeDescriptor) -> &lir::RefScan {
    let lir::TypeDescriptorScan::Fixed(scan) = &descriptor.scan else {
        panic!("expected a fixed descriptor scan")
    };
    scan
}

pub(super) fn array_scan(descriptor: &lir::TypeDescriptor) -> &lir::RefScan {
    let lir::TypeDescriptorScan::ArrayElement { scan, .. } = &descriptor.scan else {
        panic!("expected an array descriptor scan")
    };
    scan
}

pub(super) fn array_metadata<'a>(module: &'a lir::Module, name: &str) -> &'a lir::ArrayType {
    module
        .meta
        .arrays
        .iter()
        .find_map(|(_, array)| {
            (descriptor(module, array.type_descriptor).name == name).then_some(array)
        })
        .unwrap_or_else(|| panic!("missing array metadata for {name}"))
}

pub(super) fn local(name: &str, ty: mir::Type) -> mir::Local {
    mir::Local {
        name: name.to_string(),
        ty,
        mutable: false,
    }
}

pub(super) fn var(name: &str, ty: mir::Type) -> mir::Local {
    mir::Local {
        name: name.to_string(),
        ty,
        mutable: true,
    }
}

pub(super) fn stmt(kind: mir::StatementKind) -> mir::Statement {
    mir::Statement { kind, span: SPAN }
}

pub(super) fn val_decl(local: mir::LocalId, init: mir::Expr) -> mir::Statement {
    stmt(mir::StatementKind::ValDecl { local, init })
}

pub(super) fn assign(local: mir::LocalId, value: mir::Expr) -> mir::Statement {
    stmt(mir::StatementKind::Assign { local, value })
}

pub(super) fn expr_stmt(expr: mir::Expr) -> mir::Statement {
    stmt(mir::StatementKind::Expr(expr))
}

pub(super) fn expr(ty: mir::Type, kind: mir::ExprKind) -> mir::Expr {
    mir::Expr::new(ty, kind)
}

pub(super) fn local_expr(local: mir::LocalId, ty: mir::Type) -> mir::Expr {
    mir::Expr::local(local, ty)
}

pub(super) fn string_expr(id: mir::StringConstId) -> mir::Expr {
    expr(mir::Type::String, mir::ExprKind::StringConst(id))
}

pub(super) fn binary(op: mir::BinOp, lhs: mir::Expr, rhs: mir::Expr, ty: mir::Type) -> mir::Expr {
    expr(
        ty,
        mir::ExprKind::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
    )
}

pub(super) fn integer_binary_expr(
    kind: mir::IntegerKind,
    operator: mir::IntegerBinaryOperator,
    lhs: mir::Expr,
    rhs: mir::Expr,
) -> mir::Expr {
    mir::Expr::integer_binary(mir::IntegerBinaryOperation::new(kind, operator), lhs, rhs)
}

pub(super) fn integer_compare_expr(
    kind: mir::IntegerKind,
    operator: mir::IntegerComparisonOperator,
    lhs: mir::Expr,
    rhs: mir::Expr,
) -> mir::Expr {
    mir::Expr::integer_compare(
        mir::IntegerComparisonOperation::new(kind, operator),
        lhs,
        rhs,
    )
}

pub(super) fn call_symbol(module: &lir::Module, destination: lir::CallDestination) -> &str {
    match destination {
        lir::CallDestination::Local(id) => &module.functions[id.into_u32() as usize].symbol,
        lir::CallDestination::Runtime(runtime) => runtime.symbol(),
        lir::CallDestination::Extern(id) => &module.extern_functions[id].native_symbol,
        lir::CallDestination::Dispatch { .. } => panic!("dispatch calls have no symbol"),
    }
}

pub(super) fn instructions_without_polls(block: &lir::BasicBlock) -> Vec<&lir::Instruction> {
    block
        .instructions
        .iter()
        .filter(|instruction| !matches!(instruction, lir::Instruction::ManagedPoll { .. }))
        .collect()
}

pub(super) fn runtime_call(function: mir::RuntimeFn, args: Vec<mir::Expr>) -> mir::Call {
    mir::Call {
        target: mir::CallTarget {
            kind: mir::CallKind::Direct,
            callee: mir::Callee::Runtime(function),
        },
        args,
    }
}

pub(super) fn extern_call(function: mir::ExternFunctionId, args: Vec<mir::Expr>) -> mir::Call {
    mir::Call {
        target: mir::CallTarget {
            kind: mir::CallKind::Direct,
            callee: mir::Callee::Extern(function),
        },
        args,
    }
}

pub(super) fn user_call(function: mir::FunctionId) -> mir::Call {
    mir::Call {
        target: mir::CallTarget {
            kind: mir::CallKind::Direct,
            callee: mir::Callee::User(function),
        },
        args: Vec::new(),
    }
}

pub(super) fn call_stmt(call: mir::Call) -> mir::Statement {
    stmt(mir::StatementKind::Call(mir::CallEffect::Unit(call)))
}

pub(super) fn call_value(destination: mir::LocalId, call: mir::Call) -> mir::Statement {
    stmt(mir::StatementKind::Call(mir::CallEffect::Value {
        destination,
        call,
    }))
}

pub(super) fn param(name: &str, ty: mir::Type, local: mir::LocalId) -> mir::Param {
    mir::Param {
        name: name.to_string(),
        ty,
        local,
    }
}

pub(super) fn body_with_terminator(
    locals: Arena<mir::Local>,
    statements: Vec<mir::Statement>,
    terminator: mir::Terminator,
) -> mir::Body {
    let mut blocks = Arena::new();
    let entry = blocks.alloc(mir::BasicBlock {
        name: "entry".to_string(),
        statements,
        terminator,
        unwind: None,
    });
    mir::Body {
        locals,
        blocks,
        entry,
        loop_header_polls: Vec::new(),
    }
}

pub(super) fn returning_body(locals: Arena<mir::Local>, value: mir::Expr) -> mir::Body {
    body_with_terminator(
        locals,
        Vec::new(),
        mir::Terminator::Return { value: Some(value) },
    )
}

pub(super) fn cfg_block(blocks: &mut Arena<mir::BasicBlock>, name: &str) -> mir::BlockId {
    blocks.alloc(mir::BasicBlock {
        name: name.to_string(),
        statements: Vec::new(),
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    })
}

pub(super) fn set_cfg_block(
    blocks: &mut Arena<mir::BasicBlock>,
    block: mir::BlockId,
    statements: Vec<mir::Statement>,
    terminator: mir::Terminator,
    unwind: Option<mir::BlockId>,
) {
    blocks[block].statements = statements;
    blocks[block].terminator = terminator;
    blocks[block].unwind = unwind;
}

pub(super) fn cfg_block_named(body: &mir::Body, name: &str) -> mir::BlockId {
    body.blocks
        .iter()
        .find_map(|(id, block)| (block.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing MIR block `{name}`"))
}

pub(super) fn single_catch_body(
    locals: Arena<mir::Local>,
    catch_local: mir::LocalId,
    catch_ty: mir::Type,
    body_statements: Vec<mir::Statement>,
    body_terminator: Option<mir::Terminator>,
    catch_statements: Vec<mir::Statement>,
) -> mir::Body {
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let unwind = cfg_block(&mut blocks, "try.unwind.1");
    let dispatch = cfg_block(&mut blocks, "try.dispatch.2");
    let handler_pad = cfg_block(&mut blocks, "try.handler_pad.3");
    let handler_cleanup = cfg_block(&mut blocks, "try.handler_cleanup.4");
    let exit_pad = cfg_block(&mut blocks, "try.exit_pad.5");
    let exit_cleanup = cfg_block(&mut blocks, "try.exit_cleanup.6");
    let end = cfg_block(&mut blocks, "try.end.7");
    let try_body = cfg_block(&mut blocks, "try.body.8");
    let catch = cfg_block(&mut blocks, "try.catch.9");
    let next = cfg_block(&mut blocks, "try.next.10");

    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Goto(try_body),
        None,
    );
    set_cfg_block(
        &mut blocks,
        unwind,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: false,
        }))],
        mir::Terminator::Goto(dispatch),
        None,
    );
    set_cfg_block(
        &mut blocks,
        dispatch,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::BeginCatch))],
        mir::Terminator::Branch {
            cond: expr(
                mir::Type::Boolean,
                mir::ExprKind::IsInstance {
                    operand: Box::new(mir::Expr::caught_exception()),
                    check_ty: Box::new(catch_ty.clone()),
                },
            ),
            then_block: catch,
            else_block: next,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        handler_pad,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: true,
        }))],
        mir::Terminator::Goto(handler_cleanup),
        None,
    );
    set_cfg_block(
        &mut blocks,
        handler_cleanup,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
        mir::Terminator::Resume,
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit_pad,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: true,
        }))],
        mir::Terminator::Goto(exit_cleanup),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit_cleanup,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
        mir::Terminator::Resume,
        None,
    );
    set_cfg_block(
        &mut blocks,
        end,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    set_cfg_block(
        &mut blocks,
        try_body,
        body_statements,
        body_terminator.unwrap_or(mir::Terminator::Goto(end)),
        Some(unwind),
    );
    let mut catch_body = vec![val_decl(
        catch_local,
        expr(
            catch_ty.clone(),
            mir::ExprKind::Retype {
                operand: Box::new(mir::Expr::caught_exception()),
                ty: Box::new(catch_ty),
            },
        ),
    )];
    catch_body.extend(catch_statements);
    catch_body.push(stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch)));
    set_cfg_block(
        &mut blocks,
        catch,
        catch_body,
        mir::Terminator::Goto(end),
        Some(handler_pad),
    );
    set_cfg_block(
        &mut blocks,
        next,
        Vec::new(),
        mir::Terminator::Rethrow {
            unwind: Some(exit_pad),
        },
        None,
    );

    mir::Body {
        locals,
        blocks,
        entry,
        loop_header_polls: Vec::new(),
    }
}

pub(super) fn empty_vtable() -> Vec<mir::TableSlot> {
    Vec::new()
}
