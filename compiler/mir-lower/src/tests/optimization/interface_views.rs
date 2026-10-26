use super::*;

mod storage;

#[derive(Clone, Copy)]
enum Source {
    Child,
    Any,
    Final,
}

fn sample(source: Source) -> (mir::Module, mir::FunctionId) {
    let mut h = Harness::new();
    let parent = h.interface("Parent", &["read"]);
    let child = h.interface("Child", &["own"]);
    let parent_ty = h.interface_application(parent, Vec::new());
    let parent_ty = h.interface_applications[parent_ty].canonical_type;
    h.interfaces[child].parents = vec![parent_ty];
    let child_ty = h.interface_application(child, Vec::new());
    let child_ty = h.interface_applications[child_ty].canonical_type;
    let source_ty = match source {
        Source::Child => child_ty,
        Source::Any => h.any(),
        Source::Final => {
            let class = h.class("Final", hir::ClassModifier::Final, &[], None, &[parent]);
            let ty = h.class_ty(class);
            let mut locals = Arena::new();
            let this = locals.alloc(local("this", ty));
            h.method_fn(
                "Final.read",
                ty,
                vec![param("this", ty, this)],
                h.unit,
                hir::Body {
                    locals,
                    statements: Vec::new(),
                },
            );
            ty
        }
    };
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", parent_ty));
    let sink = h.user_fn_full(
        "sink",
        Vec::new(),
        vec![param("input", parent_ty, input)],
        h.unit,
        hir::Body {
            locals,
            statements: Vec::new(),
        },
    );
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", source_ty));
    locals[value].mutable = true;
    let replacement = locals.alloc(local("replacement", source_ty));
    let saved_ty = if matches!(source, Source::Any) {
        child_ty
    } else {
        source_ty
    };
    let saved = locals.alloc(local("saved", saved_ty));
    let mut statements = vec![
        val_decl(saved, local_ref(value, saved_ty)),
        expr_stmt(call(&h, sink, vec![local_ref(saved, parent_ty)])),
        expr_stmt(call(&h, sink, vec![local_ref(value, parent_ty)])),
    ];
    if matches!(source, Source::Any) {
        statements = vec![stmt(hir::StatementKind::If {
            cond: expr(
                hir::ExprKind::IsInstance {
                    operand: Box::new(local_ref(value, source_ty)),
                    check_ty: child_ty,
                },
                h.boolean,
            ),
            then_body: statements,
            else_body: None,
        })];
    }
    h.user_fn_full(
        "scope",
        Vec::new(),
        vec![
            param("value", source_ty, value),
            param("replacement", source_ty, replacement),
        ],
        h.unit,
        hir::Body { locals, statements },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let module = lower(&h.finish(main));
    let function = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "scope")
        .unwrap()
        .0;
    (module, function)
}

fn reuse(module: &mut mir::Module) -> (usize, bool) {
    let selected = mir::SelectedExternalMirSet::empty(module.cone);
    let outcome = crate::optimization::run(
        module,
        &selected,
        MirOptimizationOptions {
            interface_views: true,
            ..Default::default()
        },
    );
    assert_mir_foundation_projection(module);
    outcome
}

fn conversions(body: &mir::Body) -> Vec<mir::Expr> {
    let mut result = Vec::new();
    for (_, block) in body.blocks.iter() {
        mir::visit_block_exprs(block, &mut |expression| {
            if matches!(expression.ty, mir::Type::Interface(_))
                && matches!(expression.kind, mir::ExprKind::Retype { .. })
            {
                result.push(expression.clone());
            }
        });
    }
    result
}

fn view_sources(body: &mir::Body) -> Vec<mir::LocalId> {
    conversions(body)
        .iter()
        .map(|expression| match &expression.kind {
            mir::ExprKind::Retype { operand, .. } => match operand.kind {
                mir::ExprKind::Local(local) => local,
                _ => panic!("the view retains its concrete local source"),
            },
            _ => unreachable!(),
        })
        .collect()
}

fn retype(value: mir::Expr, ty: mir::Type) -> mir::Expr {
    mir::Expr::new(
        ty.clone(),
        mir::ExprKind::Retype {
            operand: Box::new(value),
            ty: Box::new(ty),
        },
    )
}

fn call_mut(effect: &mut mir::CallEffect) -> &mut mir::Call {
    match effect {
        mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => call,
    }
}

#[test]
fn reference_copies_fold_without_materialization_and_converge() {
    let (mut module, scope) = sample(Source::Child);
    let before = module.functions[scope].body.locals.len();
    assert_eq!(conversions(&module.functions[scope].body).len(), 2);
    let selected = mir::SelectedExternalMirSet::empty(module.cone);
    assert_eq!(
        crate::optimization::run(&mut module, &selected, Default::default()),
        (0, true)
    );
    assert_eq!(conversions(&module.functions[scope].body).len(), 2);
    assert_eq!(reuse(&mut module), (2, true));
    let body = &module.functions[scope].body;
    assert_eq!(conversions(body).len(), 2);
    assert_eq!(
        view_sources(body),
        vec![module.functions[scope].params[0].local; 2]
    );
    assert_eq!(body.locals.len(), before);
    assert_eq!(reuse(&mut module), (1, true));
    assert_eq!(module.functions[scope].body.locals.len(), before);
}

#[test]
fn smart_cast_branch_preserves_a_child_view_for_parent_projection() {
    let (mut module, scope) = sample(Source::Any);
    assert_eq!(conversions(&module.functions[scope].body).len(), 3);
    reuse(&mut module);
    let body = &module.functions[scope].body;
    let converted = conversions(body);
    assert_eq!(converted.len(), 3);
    let mir::ExprKind::Retype { operand, .. } = &converted[0].kind else {
        unreachable!()
    };
    assert_eq!(operand.ty, mir::Type::Any);
    assert!(converted.iter().any(|view| matches!(&view.kind, mir::ExprKind::Retype { operand, .. } if matches!(operand.ty, mir::Type::Interface(_)))));
    let mut entry_conversions = 0;
    mir::visit_block_exprs(&body.blocks[body.entry], &mut |expression| {
        entry_conversions += usize::from(matches!(expression.kind, mir::ExprKind::Retype { .. }));
    });
    assert_eq!(entry_conversions, 0);
    assert!(matches!(
        body.blocks[body.entry].terminator,
        mir::Terminator::Branch { .. }
    ));
}

#[test]
fn final_class_information_survives_view_folding() {
    let (mut module, scope) = sample(Source::Final);
    let final_ty = module.functions[scope].params[0].ty.clone();
    let body = &mut module.functions[scope].body;
    for (_, block) in body.blocks.iter_mut() {
        for statement in &mut block.statements {
            if let mir::StatementKind::Call(effect) = &mut statement.kind {
                let call = call_mut(effect);
                let mir::ExprKind::Retype { operand, .. } = &mut call.args[0].kind else {
                    panic!("the call converts its final receiver")
                };
                **operand = retype((**operand).clone(), mir::Type::Any);
            }
        }
    }
    reuse(&mut module);
    let converted = conversions(&module.functions[scope].body);
    assert_eq!(converted.len(), 2);
    assert!(
        matches!(&converted[0].kind, mir::ExprKind::Retype { operand, .. } if operand.ty == final_ty)
    );
}
