use super::*;

mod control;
mod retention;

fn sample(base_field: bool) -> (mir::Module, mir::FunctionId) {
    let mut h = Harness::new();
    let (int, string, boolean) = (h.int, h.string, h.boolean);
    let base = h.class(
        "Base",
        hir::ClassModifier::Open,
        &[("text", string)],
        None,
        &[],
    );
    let class = h.class(
        "Cell",
        hir::ClassModifier::Final,
        &[("number", int)],
        Some((base, vec![str_lit(&h, "initial")])),
        &[],
    );
    let ty = h.class_ty(class);
    let constructor = h
        .class_constructor_applications
        .iter()
        .find_map(|(id, application)| {
            (application.constructor
                == hir::ClassConstructorDefinition::Local(h.classes[class].constructors[0]))
            .then_some(id)
        })
        .unwrap();
    let result_ty = if base_field { string } else { int };
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", int));
    let replacement = locals.alloc(local("replacement", int));
    let text = locals.alloc(local("text", string));
    let replacement_text = locals.alloc(local("replacement_text", string));
    let external = locals.alloc(local("external", ty));
    let flag = locals.alloc(local("flag", boolean));
    let object = locals.alloc(local("object", ty));
    let alias = locals.alloc(local("alias", ty));
    let before = locals.alloc(local("before", result_ty));
    let initial = if base_field {
        local_ref(text, string)
    } else {
        local_ref(input, int)
    };
    h.user_fn_full(
        "scalar",
        Vec::new(),
        vec![
            param("input", int, input),
            param("replacement", int, replacement),
            param("text", string, text),
            param("replacement_text", string, replacement_text),
            param("external", ty, external),
            param("flag", boolean, flag),
        ],
        result_ty,
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
                val_decl(alias, local_ref(object, ty)),
                val_decl(before, initial),
                stmt(hir::StatementKind::Return {
                    value: local_ref(before, result_ty),
                }),
            ],
        },
    );
    let main = empty_main(&mut h);
    let mut module = lower(&h.finish(main));
    let id = module
        .functions
        .iter()
        .find(|(_, f)| f.name == "scalar")
        .unwrap()
        .0;
    let body = &mut module.functions[id].body;
    let object = named(body, "object");
    let alias = named(body, "alias");
    let before = named(body, "before");
    let mir::Type::Class(class_id) = body.locals[object].ty else {
        unreachable!()
    };
    let field = u32::from(!base_field);
    let ty = body.locals[before].ty.clone();
    body.blocks[body.entry].statements = vec![
        node(mir::StatementKind::Assign {
            local: object,
            value: mir::Expr::new(
                body.locals[object].ty.clone(),
                mir::ExprKind::ClassAlloc { class_id },
            ),
        }),
        write(value(body, "object"), 0, value(body, "text")),
        write(value(body, "object"), 1, value(body, "input")),
        node(mir::StatementKind::ValDecl {
            local: alias,
            init: value(body, "object"),
        }),
        node(mir::StatementKind::ValDecl {
            local: before,
            init: read(value(body, "alias"), field, ty),
        }),
        write(
            value(body, "alias"),
            field,
            value(
                body,
                if base_field {
                    "replacement_text"
                } else {
                    "replacement"
                },
            ),
        ),
    ];
    (module, id)
}

fn named(body: &mir::Body, name: &str) -> mir::LocalId {
    body.locals
        .iter()
        .find(|(_, local)| local.name == name)
        .unwrap()
        .0
}

fn value(body: &mir::Body, name: &str) -> mir::Expr {
    let id = named(body, name);
    mir::Expr::local(id, body.locals[id].ty.clone())
}

fn node(kind: mir::StatementKind) -> mir::Statement {
    mir::Statement {
        kind,
        span: crate::source_span(SPAN),
    }
}

fn read(receiver: mir::Expr, index: u32, ty: mir::Type) -> mir::Expr {
    mir::Expr::new(
        ty,
        mir::ExprKind::FieldAccess {
            receiver: Box::new(receiver),
            index,
        },
    )
}

fn write(object: mir::Expr, index: u32, value: mir::Expr) -> mir::Statement {
    node(mir::StatementKind::FieldSet {
        object,
        index,
        value,
    })
}

fn elide(module: &mut mir::Module) {
    let selected = mir::SelectedExternalMirSet::empty(module.cone);
    crate::optimization::run(
        module,
        &selected,
        MirOptimizationOptions {
            allocation_elision: true,
            ..Default::default()
        },
    );
    assert_mir_foundation_projection(module);
}

fn counts(function: &mir::Function) -> (usize, usize, usize) {
    let mut result = (0, 0, 0);
    for (_, block) in function.body.blocks.iter() {
        mir::visit_block_exprs(block, &mut |expression| match expression.kind {
            mir::ExprKind::ClassAlloc { .. } => result.0 += 1,
            mir::ExprKind::FieldAccess { .. } => result.1 += 1,
            _ => {}
        });
        result.2 += block
            .statements
            .iter()
            .filter(|statement| matches!(statement.kind, mir::StatementKind::FieldSet { .. }))
            .count();
    }
    result
}

#[test]
fn field_updates_preserve_earlier_value_copies_for_both_creation_forms() {
    for base_field in [false, true] {
        for current in [false, true] {
            for declaration in [false, true] {
                let (mut module, id) = sample(base_field);
                let body = &mut module.functions[id].body;
                if declaration {
                    let mir::StatementKind::Assign { local, value } =
                        body.blocks[body.entry].statements.remove(0).kind
                    else {
                        unreachable!()
                    };
                    body.blocks[body.entry]
                        .statements
                        .insert(0, node(mir::StatementKind::ValDecl { local, init: value }));
                }
                if current {
                    body.blocks[body.entry].terminator = mir::Terminator::Return {
                        value: read(
                            value(body, "object"),
                            u32::from(!base_field),
                            body.locals[named(body, "before")].ty.clone(),
                        ),
                    };
                }
                let expected = named(
                    body,
                    match (base_field, current) {
                        (false, false) => "input",
                        (false, true) => "replacement",
                        (true, false) => "text",
                        (true, true) => "replacement_text",
                    },
                );
                elide(&mut module);
                let function = &module.functions[id];
                assert_eq!(counts(function), (0, 0, 0));
                assert!(
                    matches!(&function.body.blocks[function.body.entry].terminator,
                    mir::Terminator::Return { value: mir::Expr { kind: mir::ExprKind::Local(local), .. } } if *local == expected)
                );
            }
        }
    }
}

#[test]
fn base_views_use_the_same_flattened_field_state() {
    let (mut module, id) = sample(true);
    let mir::Type::Class(class) =
        module.functions[id].body.locals[named(&module.functions[id].body, "object")].ty
    else {
        unreachable!()
    };
    let base = module.classes[class].base_class().unwrap();
    let body = &mut module.functions[id].body;
    let object = value(body, "object");
    let base_view = mir::Expr::new(
        mir::Type::Class(base),
        mir::ExprKind::Retype {
            operand: Box::new(object),
            ty: Box::new(mir::Type::Class(base)),
        },
    );
    body.blocks[body.entry].terminator = mir::Terminator::Return {
        value: read(base_view, 0, mir::Type::String),
    };
    let expected = named(body, "replacement_text");
    elide(&mut module);
    let body = &module.functions[id].body;
    assert_eq!(counts(&module.functions[id]), (0, 0, 0));
    assert!(matches!(&body.blocks[body.entry].terminator,
        mir::Terminator::Return { value: mir::Expr { kind: mir::ExprKind::Local(local), .. } } if *local == expected));
}
