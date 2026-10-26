use super::*;

#[derive(Clone, Copy)]
enum Leaf {
    Return,
    Throw,
    Recursive,
}

fn sample(kind: Leaf, depth: usize) -> mir::Module {
    let mut h = Harness::new();
    let int = h.int;
    let error = h.exception("SampleError");
    let error = h.class_ty(error);
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", int));
    let failure = locals.alloc(local("failure", error));
    let leaf = h.user_fn_full(
        "leaf",
        Vec::new(),
        vec![param("value", int, value), param("failure", error, failure)],
        int,
        hir::Body {
            locals: locals.clone(),
            statements: Vec::new(),
        },
    );
    let statement = match kind {
        Leaf::Return => stmt(hir::StatementKind::Return {
            value: local_ref(value, int),
        }),
        Leaf::Throw => stmt(hir::StatementKind::Throw(local_ref(failure, error))),
        Leaf::Recursive => stmt(hir::StatementKind::Return {
            value: call_typed(
                leaf,
                vec![local_ref(value, int), local_ref(failure, error)],
                int,
            ),
        }),
    };
    h.functions[leaf].kind = hir::FunctionKind::User(hir::Body {
        locals,
        statements: vec![statement],
    });
    let mut target = leaf;
    for index in 0..depth {
        let mut locals = Arena::new();
        let value = locals.alloc(local("value", int));
        let failure = locals.alloc(local("failure", error));
        target = h.user_fn_full(
            &format!("forward{index}"),
            Vec::new(),
            vec![param("value", int, value), param("failure", error, failure)],
            int,
            hir::Body {
                locals,
                statements: vec![stmt(hir::StatementKind::Return {
                    value: call_typed(
                        target,
                        vec![local_ref(value, int), local_ref(failure, error)],
                        int,
                    ),
                })],
            },
        );
    }
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", int));
    let failure = locals.alloc(local("failure", error));
    let caught = locals.alloc(local("caught", error));
    let guarded = h.user_fn_full(
        "guarded",
        Vec::new(),
        vec![param("value", int, value), param("failure", error, failure)],
        int,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Try(hir::Try {
                body: vec![stmt(hir::StatementKind::Return {
                    value: call_typed(
                        target,
                        vec![local_ref(value, int), local_ref(failure, error)],
                        int,
                    ),
                })],
                catches: vec![hir::CatchClause {
                    local: caught,
                    ty: error,
                    span: SPAN,
                    body: vec![stmt(hir::StatementKind::Return {
                        value: int_lit(&h, 99),
                    })],
                }],
                finally_body: Some(vec![expr_stmt(int_lit(&h, 17))]),
            }))],
        },
    );
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", int));
    let failure = locals.alloc(local("failure", error));
    h.user_fn_full(
        "scope",
        Vec::new(),
        vec![param("value", int, value), param("failure", error, failure)],
        int,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: call_typed(
                    guarded,
                    vec![local_ref(value, int), local_ref(failure, error)],
                    int,
                ),
            })],
        },
    );
    let main = empty_main(&mut h);
    lower(&h.finish(main))
}

fn named(module: &mir::Module, name: &str) -> mir::FunctionId {
    module
        .functions
        .iter()
        .find(|(_, f)| f.name == name)
        .unwrap()
        .0
}

fn guarded_call(module: &mir::Module) -> bool {
    let guarded = named(module, "guarded");
    module.functions[named(module, "scope")]
        .body
        .blocks
        .iter()
        .any(|(_, b)| {
            b.statements.iter().any(|s| {
                matches!(s.kind, mir::StatementKind::Call(_))
                    && matches!(statement_call(s).0.target,
                        mir::CallTarget::Direct(mir::Callee::User(id)) if id == guarded)
            })
        })
}

#[test]
fn proven_calls_remove_only_unreachable_handlers_and_keep_normal_finally() {
    let mut module = sample(Leaf::Return, 0);
    assert!(guarded_call(&module));
    optimize(&mut module);
    assert!(!guarded_call(&module));
    let function = &module.functions[named(&module, "scope")];
    assert!(function.body.blocks.iter().all(|(_, b)| {
        b.unwind.is_none()
            && b.statements
                .iter()
                .all(|s| !matches!(s.kind, mir::StatementKind::Eh(_)))
    }));
    assert!(
        function
            .body
            .blocks
            .iter()
            .any(|(_, b)| b.statements.iter().any(|s| matches!(
                s.kind,
                mir::StatementKind::Expr(mir::Expr {
                    kind: mir::ExprKind::IntegerLiteral(mir::MirIntegerConstant::Signed32(17)),
                    ..
                })
            )))
    );
    assert!(
        function
            .body
            .blocks
            .iter()
            .any(|(_, b)| matches!(&b.terminator,
        mir::Terminator::Return { value: mir::Expr { kind: mir::ExprKind::Local(id), .. } }
            if *id == function.params[0].local))
    );
}

#[test]
fn throwing_and_recursive_dependencies_keep_the_original_exception_call() {
    for kind in [Leaf::Throw, Leaf::Recursive] {
        let mut module = sample(kind, 0);
        optimize(&mut module);
        assert!(guarded_call(&module));
    }
}

#[test]
fn nonthrowing_helper_chains_do_not_require_transitive_inlining() {
    let mut module = sample(Leaf::Return, 48);
    optimize(&mut module);
    assert!(!guarded_call(&module));
}
