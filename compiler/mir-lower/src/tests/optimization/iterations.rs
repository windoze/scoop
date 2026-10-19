use super::*;

/// An open receiver is exact only after the caller's helper has been inlined.
fn dispatch_chain(method_count: usize, calls: usize, recursive: bool) -> mir::Module {
    let mut h = Harness::new();
    let unit = h.unit;
    let class = h.class("Dispatch", hir::ClassModifier::Open, &[], None, &[]);
    let class_ty = h.class_ty(class);
    let mut methods = Vec::new();
    for index in 0..method_count {
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", class_ty));
        methods.push(h.method_fn(
            &format!("Dispatch.step{index}"),
            class_ty,
            vec![param("this", class_ty, this)],
            unit,
            hir::Body {
                locals,
                statements: Vec::new(),
            },
        ));
    }
    let applications = methods
        .iter()
        .map(|method| h.method_application(*method))
        .collect::<Vec<_>>();
    let invoke = |receiver, application| {
        expr(
            hir::ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee: hir::MethodCallee::Callable(hir::Callable::Method(application).into()),
                args: Vec::new(),
            },
            unit,
        )
    };
    for index in 0..method_count - 1 {
        let this = h.functions[methods[index]].signature.params[0].local;
        let hir::FunctionKind::User(body) = &mut h.functions[methods[index]].kind else {
            unreachable!()
        };
        body.statements.push(expr_stmt(invoke(
            local_ref(this, class_ty),
            applications[index + 1],
        )));
    }
    let mut locals = Arena::new();
    let receiver = locals.alloc(local("receiver", class_ty));
    let helper = h.user_fn_full(
        "helper",
        Vec::new(),
        vec![param("receiver", class_ty, receiver)],
        unit,
        hir::Body {
            locals,
            statements: vec![expr_stmt(invoke(
                local_ref(receiver, class_ty),
                applications[0],
            ))],
        },
    );
    if recursive {
        let method = methods[method_count - 1];
        let this = h.functions[method].signature.params[0].local;
        let recurse = expr_stmt(call(&h, helper, vec![local_ref(this, class_ty)]));
        let hir::FunctionKind::User(body) = &mut h.functions[method].kind else {
            unreachable!()
        };
        body.statements.push(recurse);
    }
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
    let object = locals.alloc(local("object", class_ty));
    let mut statements = vec![val_decl(
        object,
        expr(
            hir::ExprKind::ClassInit {
                constructor,
                args: Vec::new(),
            },
            class_ty,
        ),
    )];
    statements
        .extend((0..calls).map(|_| expr_stmt(call(&h, helper, vec![local_ref(object, class_ty)]))));
    let main = h.user_fn("main", hir::Body { locals, statements });
    lower(&h.finish(main))
}

fn run(module: &mut mir::Module, limit: usize) -> (usize, bool) {
    let selected = mir::SelectedExternalMirSet::empty(module.cone);
    let outcome = crate::optimization::run_to_limit(
        module,
        &selected,
        MirOptimizationOptions::RELEASE,
        limit,
    );
    assert_mir_foundation_projection(module);
    outcome
}

fn main_calls(module: &mir::Module) -> Vec<&mir::Call> {
    module.functions[module.executable_entry().unwrap()]
        .body
        .blocks
        .iter()
        .flat_map(|(_, block)| &block.statements)
        .filter_map(|statement| match &statement.kind {
            mir::StatementKind::Call(
                mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. },
            ) => Some(call),
            _ => None,
        })
        .collect()
}

#[test]
fn alternating_rounds_inline_the_newly_resolved_target_and_report_convergence() {
    let mut once = dispatch_chain(1, 1, false);
    assert_eq!(run(&mut once, 1), (1, false));
    assert!(
        main_calls(&once)
            .iter()
            .any(|call| matches!(call.target, mir::CallTarget::Virtual { .. }))
    );
    let mut twice = dispatch_chain(1, 1, false);
    assert_eq!(run(&mut twice, 2), (2, false));
    assert!(main_calls(&twice).is_empty());
    let mut fixed = dispatch_chain(1, 1, false);
    assert_eq!(run(&mut fixed, 4), (3, true));
    assert!(main_calls(&fixed).is_empty());
}

#[test]
fn nested_expansion_history_survives_more_than_six_rounds() {
    let mut module = dispatch_chain(9, 1, false);
    assert!(run(&mut module, 12).1);
    let calls = main_calls(&module);
    assert_eq!(calls.len(), 1);
    let mir::CallTarget::Direct(mir::Callee::User(target)) = calls[0].target else {
        panic!("the last exposed target is direct")
    };
    assert_eq!(module.functions[target].name, "Dispatch.step5");
}

#[test]
fn caller_growth_is_not_refilled_by_later_rounds() {
    let mut module = dispatch_chain(1, 128, false);
    assert!(run(&mut module, 12).1);
    let calls = main_calls(&module);
    assert!(
        !calls.is_empty(),
        "fresh per-round growth would remove every shallow call"
    );
    assert!(calls.iter().any(|call| matches!(call.target, mir::CallTarget::Direct(mir::Callee::User(target)) if module.functions[target].name == "Dispatch.step0")));
}

#[test]
fn expansion_ancestry_stops_recursion_hidden_in_an_open_receiver() {
    let mut module = dispatch_chain(1, 1, true);
    assert!(run(&mut module, 12).1);
    let calls = main_calls(&module);
    assert_eq!(calls.len(), 1);
    let mir::CallTarget::Direct(mir::Callee::User(target)) = calls[0].target else {
        panic!("the recursive edge is resolved")
    };
    assert_eq!(module.functions[target].name, "helper");
    assert!(module.functions[target].body.blocks.len() < 16);
}

#[test]
fn newly_exposed_recursive_edges_refresh_the_call_graph() {
    let mut module = dispatch_chain(1, 1, true);
    let method = module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == "Dispatch.step0").then_some(id))
        .unwrap();
    // Model the body after an empty constructor is inlined: the next dispatch
    // becomes exact only when helper has also been expanded in the first round.
    for (_, block) in module.functions[method].body.blocks.iter_mut() {
        for statement in &mut block.statements {
            if let mir::StatementKind::Call(mir::CallEffect::Unit(call)) = &mut statement.kind {
                let mir::Type::Class(class_id) = call.args[0].ty else {
                    panic!("class receiver")
                };
                call.args[0].kind = mir::ExprKind::ClassAlloc { class_id };
            }
        }
    }
    assert!(run(&mut module, 12).1);
    let calls = main_calls(&module);
    assert_eq!(calls.len(), 1);
    assert!(
        matches!(calls[0].target, mir::CallTarget::Direct(mir::Callee::User(target)) if target == method)
    );
}
