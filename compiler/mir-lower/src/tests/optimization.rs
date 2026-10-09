use super::*;

fn optimize(module: &mut mir::Module) {
    let selected = mir::SelectedExternalMirSet::empty(module.cone);
    crate::optimization::run(module, &selected, MirOptimizationOptions::RELEASE);
    assert_mir_foundation_projection(module);
}

#[test]
fn inline_copies_have_distinct_caller_local_identities() {
    let mut h = Harness::new();
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", h.int));
    let identity = h.user_fn_full(
        "identity",
        Vec::new(),
        vec![param("value", h.int, value)],
        h.int,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(local_ref(value, h.int)),
            })],
        },
    );
    let body = hir::Body {
        locals: Arena::new(),
        statements: vec![
            expr_stmt(call_typed(identity, vec![int_lit(&h, 24)], h.int)),
            expr_stmt(call_typed(identity, vec![int_lit(&h, 31)], h.int)),
        ],
    };
    let main = h.user_fn("main", body);
    let mut module = lower(&h.finish(main));
    let main = module.executable_entry().unwrap();
    let before = module.functions[main].body.locals.len();
    let identities = module.meta.local_values.iter().cloned().collect::<Vec<_>>();
    let owner = module
        .meta
        .source_callable_materializations
        .get(main)
        .unwrap()
        .materialization();
    optimize(&mut module);
    for entry in identities {
        assert_eq!(
            module
                .meta
                .local_values
                .get_owned(entry.owner(), entry.local()),
            Some(&entry)
        );
    }
    let copies = module
        .meta
        .local_values
        .iter()
        .filter(|entry| {
            entry.owner() == main.into() && entry.local().into_raw().into_u32() as usize >= before
        })
        .collect::<Vec<_>>();
    assert_eq!(copies.len(), 4);
    let unique = copies
        .iter()
        .map(|copy| copy.identity_record().id())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), copies.len());
    assert!(
        copies
            .iter()
            .all(|copy| copy.identity_record().key().owner() == owner)
    );
    assert!(module.functions[main].body.blocks.iter().all(|(_, block)| {
        !block
            .statements
            .iter()
            .any(|statement| matches!(statement.kind, mir::StatementKind::Call(_)))
    }));
    for (_, block) in module.functions[main].body.blocks.iter() {
        for statement in &block.statements {
            if let mir::StatementKind::Assign { local, .. } = statement.kind {
                assert!(local.into_raw().into_u32() as usize >= before);
            }
        }
    }
}

#[test]
fn recursive_callee_is_retained_at_each_call_site() {
    let mut h = Harness::new();
    let recursive = h.user_fn(
        "recursive",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    h.functions[recursive].kind = hir::FunctionKind::User(hir::Body {
        locals: Arena::new(),
        statements: vec![expr_stmt(call(&h, recursive, Vec::new()))],
    });
    let body = hir::Body {
        locals: Arena::new(),
        statements: vec![expr_stmt(call(&h, recursive, Vec::new()))],
    };
    let main = h.user_fn("main", body);
    let mut module = lower(&h.finish(main));
    let recursive = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "recursive")
        .unwrap()
        .0;
    optimize(&mut module);
    for function in [recursive, module.executable_entry().unwrap()] {
        assert!(
            module.functions[function]
                .body
                .blocks
                .iter()
                .any(|(_, block)| {
                    block
                        .statements
                        .iter()
                        .any(|statement| match &statement.kind {
                            mir::StatementKind::Call(effect) => {
                                let call = match effect {
                                    mir::CallEffect::Unit(call)
                                    | mir::CallEffect::Value { call, .. } => call,
                                };
                                call.target.callee == mir::Callee::User(recursive)
                            }
                            _ => false,
                        })
                })
        );
    }
}

#[test]
fn inline_loops_preserve_the_callee_gc_effect() {
    for effect in [hir::GcEffect::NoGc, hir::GcEffect::Managed] {
        let mut h = Harness::new();
        let mut locals = Arena::new();
        let flag = locals.alloc(local("flag", h.boolean));
        let spin = h.user_fn_full(
            "spin",
            Vec::new(),
            vec![param("flag", h.boolean, flag)],
            h.unit,
            hir::Body {
                locals,
                statements: vec![stmt(hir::StatementKind::While {
                    target: hir::LoopId::from_raw(0),
                    condition_setup: Vec::new(),
                    cond: local_ref(flag, h.boolean),
                    body: Vec::new(),
                })],
            },
        );
        h.functions[spin].attributes.gc_effect = effect;
        let mut locals = Arena::new();
        let flag = locals.alloc(local("flag", h.boolean));
        let body = hir::Body {
            locals,
            statements: vec![expr_stmt(call(&h, spin, vec![local_ref(flag, h.boolean)]))],
        };
        let caller = h.user_fn_full(
            "caller",
            Vec::new(),
            vec![param("flag", h.boolean, flag)],
            h.unit,
            body,
        );
        let body = hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(call(&h, caller, vec![bool_lit(&h, false)]))],
        };
        let main = h.user_fn("main", body);
        let mut module = lower(&h.finish(main));
        let caller = module
            .functions
            .iter()
            .find(|(_, function)| function.name == "caller")
            .unwrap()
            .0;
        optimize(&mut module);
        let body = &module.functions[caller].body;
        assert!(
            body.blocks
                .iter()
                .any(|(_, block)| matches!(block.terminator, mir::Terminator::Branch { .. }))
        );
        assert!(body.blocks.iter().all(|(_, block)| {
            block
                .statements
                .iter()
                .all(|statement| !matches!(statement.kind, mir::StatementKind::Call(_)))
        }));
        assert_eq!(
            body.loop_header_polls.len(),
            usize::from(effect == hir::GcEffect::Managed)
        );
    }
}
