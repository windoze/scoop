//! Values overwritten after a call still belong to its exceptional edge.

use super::*;

fn overwritten_cleanup_value(later_call: bool) -> mir::Module {
    let mut h = Harness::new();
    let suspend = h.user_fn(
        "wait",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    h.functions[suspend].is_suspend = true;
    let ordinary = h.user_fn(
        "mayThrow",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let print = h.print_string();
    let mut locals = Arena::new();
    let prior = locals.alloc(local("prior", h.string));
    locals[prior].mutable = true;
    let mut statements = vec![val_decl(prior, str_lit(&h, "before"))];
    if later_call {
        statements.push(expr_stmt(call(&h, suspend, Vec::new())));
    }
    statements.push(stmt(hir::StatementKind::Try(hir::Try {
        body: vec![
            expr_stmt(call(
                &h,
                if later_call { ordinary } else { suspend },
                Vec::new(),
            )),
            stmt(hir::StatementKind::Assign {
                target: hir::AssignTarget::Local(prior),
                value: str_lit(&h, "after"),
            }),
        ],
        catches: Vec::new(),
        finally_body: Some(vec![expr_stmt(call(
            &h,
            print,
            vec![local_ref(prior, h.string)],
        ))]),
    })));
    let caller = h.user_fn("cleanupCaller", hir::Body { locals, statements });
    h.functions[caller].is_suspend = true;
    let main = empty_main(&mut h);
    lower(&h.finish_coroutines(main))
}

fn assert_cleanup_value_is_saved(module: &mir::Module) {
    let coroutine = module
        .meta
        .coroutine_functions
        .iter()
        .find_map(|(_, coroutine)| {
            (module.functions[coroutine.function].name == "cleanupCaller").then_some(coroutine)
        })
        .unwrap();
    let mir::CoroutineLowering::StateMachine {
        frame,
        driver,
        resume_points,
        ..
    } = &coroutine.lowering
    else {
        panic!("the caller must suspend");
    };
    let frame = &module.meta.coroutine_frames[*frame];
    let driver = &module.functions[*driver];
    let (prior, _) = driver
        .body
        .locals
        .iter()
        .find(|(_, local)| local.name == "prior")
        .unwrap();
    assert!(
        declared_class(&module.classes[frame.class()])
            .fields
            .iter()
            .any(|field| field.name == "local$prior"),
        "the old value used by finally must survive suspension"
    );
    assert_eq!(resume_points.len(), 1);
    let point = &module.meta.coroutine_resume_points[resume_points[0]];
    for entry in [
        point.success().entry().block(),
        point.failure().entry().block(),
    ] {
        assert!(driver.body.blocks[entry].statements.iter().any(|statement| matches!(
            &statement.kind,
            mir::StatementKind::Assign { local, value } if *local == prior && matches!(value.kind, mir::ExprKind::EnumField { .. })
        )), "the saved cleanup value must be restored before continuing");
    }
}

#[test]
fn suspension_preserves_cleanup_value_overwritten_on_success() {
    assert_cleanup_value_is_saved(&overwritten_cleanup_value(false));
}

#[test]
fn suspension_preserves_cleanup_value_for_a_later_ordinary_call_failure() {
    assert_cleanup_value_is_saved(&overwritten_cleanup_value(true));
}
