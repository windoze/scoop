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
                debug_origin: Default::default(),
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

#[test]
fn loop_defined_values_propagate_through_copy_chains() {
    let mut h = Harness::new();
    let boolean = h.boolean;
    let mut locals = Arena::new();
    let left = locals.alloc(local("left", boolean));
    let right = locals.alloc(local("right", boolean));
    let repeat = locals.alloc(local("repeat", boolean));
    let computed = locals.alloc(local("computed", boolean));
    let mut statements = vec![val_decl(computed, local_ref(left, boolean))];
    let mut previous = computed;
    for index in 0..8 {
        let copy = locals.alloc(local(&format!("snapshot{index}"), boolean));
        statements.push(val_decl(copy, local_ref(previous, boolean)));
        previous = copy;
    }
    statements.push(stmt(hir::StatementKind::Return {
        value: local_ref(previous, boolean),
    }));
    h.user_fn_full(
        "scope",
        Vec::new(),
        vec![
            param("left", boolean, left),
            param("right", boolean, right),
            param("repeat", boolean, repeat),
        ],
        boolean,
        hir::Body { locals, statements },
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
    let entry = function.body.entry;
    let left = function.params[0].local;
    let right = function.params[1].local;
    let repeat = function.params[2].local;
    let first = &mut function.body.blocks[entry].statements[0];
    let mir::StatementKind::ValDecl {
        local: computed,
        init,
    } = &mut first.kind
    else {
        panic!("computed local")
    };
    let computed = *computed;
    *init = mir::Expr::new(
        mir::Type::Boolean,
        mir::ExprKind::Binary {
            op: mir::BinOp::BoolEq,
            lhs: Box::new(mir::Expr::local(left, mir::Type::Boolean)),
            rhs: Box::new(mir::Expr::local(right, mir::Type::Boolean)),
        },
    );
    let mut exit = function.body.blocks[entry].clone();
    exit.name = "exit".into();
    exit.statements.clear();
    let exit = function.body.blocks.alloc(exit);
    function.body.blocks[entry].terminator = mir::Terminator::Branch {
        cond: mir::Expr::local(repeat, mir::Type::Boolean),
        then_block: entry,
        else_block: exit,
    };
    optimize(&mut module);
    let function = &module.functions[scope];
    assert!(
        matches!(&function.body.blocks[exit].terminator, mir::Terminator::Return { value: mir::Expr { kind: mir::ExprKind::Local(local), .. } } if *local == computed)
    );
    assert_eq!(function.body.blocks[entry].statements.len(), 1);
}
