use super::*;

mod retention;

fn program() -> (mir::Module, mir::FunctionId, mir::FunctionId) {
    let mut h = Harness::new();
    let (int, unit, boolean) = (h.int, h.unit, h.boolean);
    let class = h.class(
        "Accumulator",
        hir::ClassModifier::Final,
        &[("value", int)],
        None,
        &[],
    );
    let ty = h.class_ty(class);
    let mut locals = Arena::new();
    let object = locals.alloc(local("object", ty));
    let opaque = h.user_fn_full(
        "opaque",
        Vec::new(),
        vec![param("object", ty, object)],
        unit,
        hir::Body {
            locals,
            statements: Vec::new(),
        },
    );
    let recurse = expr_stmt(call_typed(opaque, vec![local_ref(object, ty)], unit));
    let hir::FunctionKind::User(body) = &mut h.functions[opaque].kind else {
        unreachable!()
    };
    body.statements.push(recurse);

    let mut locals = Arena::new();
    let this = locals.alloc(local("this", ty));
    let foreign = locals.alloc(local("foreign", ty));
    let flag = locals.alloc(local("flag", boolean));
    let alias = locals.alloc(local("alias", ty));
    let before = locals.alloc(local("before", int));
    let method = h.method_fn(
        "Accumulator.advance",
        ty,
        vec![
            param("this", ty, this),
            param("foreign", ty, foreign),
            param("flag", boolean, flag),
        ],
        int,
        hir::Body {
            locals,
            statements: vec![
                val_decl(alias, local_ref(this, ty)),
                val_decl(before, int_lit(&h, 0)),
                stmt(hir::StatementKind::Return {
                    value: local_ref(before, int),
                }),
            ],
        },
    );
    let application = h.method_application(method);
    let constructor = h
        .class_constructor_applications
        .iter()
        .find_map(|(id, app)| {
            (app.constructor
                == hir::ClassConstructorDefinition::Local(h.classes[class].constructors[0]))
            .then_some(id)
        })
        .unwrap();
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", int));
    let foreign = locals.alloc(local("foreign", ty));
    let flag = locals.alloc(local("flag", boolean));
    let object = locals.alloc(local("object", ty));
    h.user_fn_full(
        "scope",
        Vec::new(),
        vec![
            param("input", int, input),
            param("foreign", ty, foreign),
            param("flag", boolean, flag),
        ],
        int,
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    object,
                    expr(
                        hir::ExprKind::ClassInit {
                            constructor,
                            args: vec![local_ref(input, int)],
                        },
                        ty,
                    ),
                ),
                stmt(hir::StatementKind::Return {
                    value: expr(
                        hir::ExprKind::MethodCall {
                            receiver: Box::new(local_ref(object, ty)),
                            callee: hir::MethodCallee::Callable(
                                hir::Callable::Method(application).into(),
                            ),
                            args: vec![local_ref(foreign, ty), local_ref(flag, boolean)],
                        },
                        int,
                    ),
                }),
            ],
        },
    );
    let main = empty_main(&mut h);
    let mut module = lower(&h.finish(main));
    let find = |name| {
        module
            .functions
            .iter()
            .find(|(_, f)| f.name == name)
            .unwrap()
            .0
    };
    let (scope, method) = (find("scope"), find("Accumulator.advance"));
    let body = &mut module.functions[method].body;
    let alias = named(body, "alias");
    let before = named(body, "before");
    let ty = body.locals[before].ty.clone();
    let mut statements = vec![
        node(mir::StatementKind::ValDecl {
            local: alias,
            init: value(body, "this"),
        }),
        node(mir::StatementKind::ValDecl {
            local: before,
            init: read(value(body, "alias"), 0, ty.clone()),
        }),
    ];
    for _ in 0..6 {
        statements.push(write(
            value(body, "alias"),
            0,
            mir::Expr::integer_binary(
                mir::IntegerBinaryOperation::new(
                    mir::IntegerKind::SIGNED_32,
                    mir::IntegerBinaryOperator::Add,
                ),
                read(value(body, "alias"), 0, ty.clone()),
                mir::Expr::integer(mir::MirIntegerConstant::Signed32(1)),
            ),
        ));
    }
    body.blocks[body.entry].statements = statements;
    (module, scope, method)
}

fn run(module: &mut mir::Module, enabled: bool) {
    let selected = mir::SelectedExternalMirSet::empty(module.cone);
    crate::optimization::run(
        module,
        &selected,
        MirOptimizationOptions {
            allocation_elision: enabled,
            ..MirOptimizationOptions::RELEASE
        },
    );
    assert_mir_foundation_projection(module);
}

fn method_calls(module: &mir::Module, scope: mir::FunctionId, method: mir::FunctionId) -> usize {
    module.functions[scope]
        .body
        .blocks
        .iter()
        .flat_map(|(_, block)| &block.statements)
        .filter(|s| {
            matches!(&s.kind, mir::StatementKind::Call(_)
            if matches!(statement_call(s).0.target,
                mir::CallTarget::Direct(mir::Callee::User(id))
                | mir::CallTarget::Virtual { callee: mir::DispatchCallee::User(id), .. }
                if id == method))
        })
        .count()
}

#[test]
fn last_receiver_call_uses_projection_cost_only_when_elision_is_enabled() {
    for enabled in [false, true] {
        let (mut module, scope, method) = program();
        run(&mut module, enabled);
        assert_eq!(method_calls(&module, scope, method), usize::from(!enabled));
        assert_eq!(counts(&module.functions[scope]).0, usize::from(!enabled));
        if enabled {
            assert_eq!(counts(&module.functions[scope]), (0, 0, 0));
            let body = &module.functions[scope].body;
            let input = named(body, "input");
            assert!(
                body.blocks
                    .iter()
                    .any(|(_, block)| matches!(&block.terminator,
                mir::Terminator::Return { value: mir::Expr { kind: mir::ExprKind::Local(id), .. } }
                    if *id == input))
            );
        }
    }
}
