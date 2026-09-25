use super::*;

#[test]
fn smart_cast_unboxes_bind_typed_hidden_locals() {
    // `if (a is S) { println(a.v) }` — the narrowed read arrives as
    // `FieldAccess { receiver: Unbox(Local a) }` (hir-lower's smart
    // cast). The unbox must be bound to a typed hidden local so LIR
    // never has to reconstruct its type from the `Any` operand.
    let mut h = Harness::new();
    let println_int = h.println_int();
    let (int, boolean) = (h.int, h.boolean);
    let s = h.strukt("S", &[("v", int)]);
    let s_ty = h.struct_ty(s);
    let s_application = h.struct_application_of(s_ty);
    let any = h.any();
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", any));
    let print_call = expr(
        hir::ExprKind::Call {
            receiver: scoop_hir::SourceCallReceiver::NoReceiver,
            callee: hir::Callable::Function(println_int),
            args: vec![expr(
                hir::ExprKind::FieldAccess {
                    receiver: Box::new(expr(
                        hir::ExprKind::Unbox(Box::new(local_ref(a, any))),
                        s_ty,
                    )),
                    field: h.struct_field_ref(s_application, 0),
                },
                int,
            )],
        },
        h.unit,
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::If {
                cond: expr(
                    hir::ExprKind::IsInstance {
                        operand: Box::new(local_ref(a, any)),
                        check_ty: s_ty,
                    },
                    boolean,
                ),
                then_body: vec![expr_stmt(print_call)],
                else_body: None,
            })],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module
        .output
        .executable_entry()
        .expect("test module is executable")]
    .body;
    let mir::Terminator::Branch { then_block, .. } = body.blocks[body.entry].terminator else {
        panic!("expected a conditional branch")
    };
    let then_body = &body.blocks[then_block].statements;
    let mir::StatementKind::ValDecl { local: ub, init } = &then_body[0].kind else {
        panic!("the unbox must be a val declaration")
    };
    let mir::ExprKind::Unbox(_) = init.kind else {
        panic!("the unbox must be bound to a typed hidden local")
    };
    assert_eq!(
        body.locals[*ub].ty,
        mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    let (call, _) = statement_call(&then_body[1]);
    assert!(
        matches!(&call.args[0].kind, mir::ExprKind::FieldAccess { receiver, .. }
            if matches!(receiver.kind, mir::ExprKind::Local(local) if local == *ub))
    );
}
