use super::*;

fn function_named(module: &mir::Module, name: &str) -> mir::FunctionId {
    module
        .functions
        .iter()
        .find(|(_, function)| function.name == name)
        .unwrap()
        .0
}

#[test]
fn parameter_copy_chains_do_not_keep_a_small_helper_out_of_line() {
    let mut h = Harness::new();
    let int = h.int;
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", int));
    let mut previous = value;
    let mut statements = Vec::new();
    for index in 0..40 {
        let copy = locals.alloc(local(&format!("copy{index}"), int));
        statements.push(val_decl(copy, local_ref(previous, int)));
        previous = copy;
    }
    statements.push(stmt(hir::StatementKind::Return {
        value: local_ref(previous, int),
    }));
    let helper = h.user_fn_full(
        "helper",
        Vec::new(),
        vec![param("value", int, value)],
        int,
        hir::Body { locals, statements },
    );
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", int));
    h.user_fn_full(
        "scope",
        Vec::new(),
        vec![param("value", int, value)],
        int,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: call_typed(helper, vec![local_ref(value, int)], int),
            })],
        },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let mut module = lower(&h.finish(main));
    optimize(&mut module);
    let function = &module.functions[function_named(&module, "scope")];
    assert!(function.body.blocks.iter().all(|(_, block)| {
        block
            .statements
            .iter()
            .all(|statement| !matches!(statement.kind, mir::StatementKind::Call(_)))
    }));
    let value = function.params[0].local;
    assert!(function.body.blocks.iter().any(|(_, block)| matches!(block.terminator,
        mir::Terminator::Return { value: mir::Expr { kind: mir::ExprKind::Local(local), .. } } if local == value)));
}

#[test]
fn a_mutated_or_addressed_source_does_not_replace_its_snapshot() {
    for addressed in [false, true] {
        let mut h = Harness::new();
        let int = h.int;
        let mut locals = Arena::new();
        let value = locals.alloc(local("value", int));
        let replacement = locals.alloc(local("replacement", int));
        let snapshot = locals.alloc(local("snapshot", int));
        h.user_fn_full(
            "scope",
            Vec::new(),
            vec![
                param("value", int, value),
                param("replacement", int, replacement),
            ],
            int,
            hir::Body {
                locals,
                statements: vec![
                    val_decl(snapshot, local_ref(value, int)),
                    stmt(hir::StatementKind::Return {
                        value: local_ref(snapshot, int),
                    }),
                ],
            },
        );
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            },
        );
        let mut module = lower(&h.finish(main));
        let scope = function_named(&module, "scope");
        let function = &mut module.functions[scope];
        let value = function.params[0].local;
        let replacement = function.params[1].local;
        let snapshot = function
            .body
            .locals
            .iter()
            .find(|(_, local)| local.name == "snapshot")
            .unwrap()
            .0;
        let ty = function.params[0].ty.clone();
        let kind = if addressed {
            mir::StatementKind::Expr(mir::Expr::new(
                mir::Type::Ptr(Box::new(ty.clone())),
                mir::ExprKind::AddressOf {
                    local: value,
                    pointee: Box::new(ty.clone()),
                },
            ))
        } else {
            mir::StatementKind::Assign {
                local: value,
                value: mir::Expr::local(replacement, ty),
            }
        };
        function.body.blocks[function.body.entry]
            .statements
            .push(mir::Statement {
                kind,
                span: crate::source_span(SPAN),
            });
        let selected = mir::SelectedExternalMirSet::empty(module.cone);
        crate::optimization::run(
            &mut module,
            &selected,
            MirOptimizationOptions {
                inline: true,
                ..Default::default()
            },
        );
        let body = &module.functions[scope].body;
        assert!(matches!(body.blocks[body.entry].terminator,
            mir::Terminator::Return { value: mir::Expr { kind: mir::ExprKind::Local(local), .. } } if local == snapshot));
        assert!(body.blocks[body.entry].statements.iter().any(|statement| matches!(statement.kind, mir::StatementKind::ValDecl { local, .. } if local == snapshot)));
    }
}
