use super::*;

fn interface_box() -> mir::Module {
    let mut h = Harness::new();
    let interface = h.interface("Visit", &["visit"]);
    let interface_ty = h.interface_ty(interface);
    let int = h.int;
    let value_type = h.strukt_with("Value", &[("number", int)], &[interface]);
    let value_ty = h.struct_ty(value_type);
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", value_ty));
    h.method_fn(
        "Value.visit",
        value_ty,
        vec![param("this", value_ty, this)],
        h.unit,
        hir::Body {
            locals,
            statements: Vec::new(),
        },
    );
    let method = h.interface_methods[h.interfaces[interface].methods[0]].function;
    let method = h.method_application(method);
    let mut locals = Arena::new();
    let parameter = locals.alloc(local("parameter", interface_ty));
    let helper = h.user_fn_full(
        "visit",
        Vec::new(),
        vec![param("parameter", interface_ty, parameter)],
        h.unit,
        hir::Body {
            locals,
            statements: vec![expr_stmt(expr(
                hir::ExprKind::MethodCall {
                    receiver: Box::new(local_ref(parameter, interface_ty)),
                    callee: hir::MethodCallee::Callable(hir::Callable::Method(method).into()),
                    args: Vec::new(),
                },
                h.unit,
            ))],
        },
    );
    let mut locals = Arena::new();
    let boxed = locals.alloc(local("boxed", interface_ty));
    let body = hir::Body {
        locals,
        statements: vec![
            val_decl(
                boxed,
                expr(
                    hir::ExprKind::Box(Box::new(struct_init(&h, value_ty, vec![int_lit(&h, 17)]))),
                    interface_ty,
                ),
            ),
            expr_stmt(expr(
                hir::ExprKind::IsInstance {
                    operand: Box::new(local_ref(boxed, interface_ty)),
                    check_ty: interface_ty,
                },
                h.boolean,
            )),
            expr_stmt(call(&h, helper, vec![local_ref(boxed, interface_ty)])),
        ],
    };
    let main = h.user_fn("main", body);
    lower(&h.finish(main))
}

#[test]
fn dispatch_and_inline_remove_the_unknown_call_before_box_elision() {
    let mut once = interface_box();
    let selected = mir::SelectedExternalMirSet::empty(once.cone);
    crate::optimization::run_to_limit(&mut once, &selected, MirOptimizationOptions::RELEASE, 1);
    let main = once.executable_entry().unwrap();
    assert_eq!(counts(&once.functions[main]), (1, 0, 0));
    assert!(once.functions[main].body.blocks.iter().any(|(_, block)| {
        block
            .statements
            .iter()
            .any(|statement| matches!(statement.kind, mir::StatementKind::Call(_)))
    }));

    let mut complete = interface_box();
    optimize(&mut complete);
    let main = &complete.functions[complete.executable_entry().unwrap()];
    assert_eq!(counts(main), (0, 0, 0));
    assert!(main.body.blocks.iter().all(|(_, block)| {
        block
            .statements
            .iter()
            .all(|statement| !matches!(statement.kind, mir::StatementKind::Call(_)))
    }));
}
