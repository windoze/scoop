use super::*;
use mir::optimization_report::{InlineOutcome, OptimizationDecision};

#[test]
fn nested_calls_receive_growth_before_setup_and_outer_calls() {
    let mut h = Harness::new();
    let (unit, boolean) = (h.unit, h.boolean);
    let observe = h.user_fn(
        "observe",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let routines = ["setup", "outer", "nested"].map(|name| {
        h.user_fn(
            name,
            hir::Body {
                locals: Arena::new(),
                statements: (0..3)
                    .map(|_| expr_stmt(call_typed(observe, Vec::new(), unit)))
                    .collect(),
            },
        )
    });
    let mut locals = Arena::new();
    let enter = locals.alloc(local("enter", boolean));
    let mut statements = (0..24)
        .map(|_| expr_stmt(call_typed(routines[0], Vec::new(), unit)))
        .collect::<Vec<_>>();
    statements.push(stmt(hir::StatementKind::While {
        loop_kind: hir::LoopKind::While,
        target: hir::LoopId::from_raw(0),
        condition_setup: Vec::new(),
        cond: local_ref(enter, boolean),
        body: vec![
            expr_stmt(call_typed(routines[1], Vec::new(), unit)),
            stmt(hir::StatementKind::While {
                loop_kind: hir::LoopKind::While,
                target: hir::LoopId::from_raw(1),
                condition_setup: Vec::new(),
                cond: local_ref(enter, boolean),
                body: vec![expr_stmt(call_typed(routines[2], Vec::new(), unit))],
            }),
        ],
    }));
    h.user_fn_full(
        "scope",
        Vec::new(),
        vec![param("enter", boolean, enter)],
        unit,
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
    let find = |name| {
        module
            .functions
            .iter()
            .find(|(_, f)| f.name == name)
            .unwrap()
            .0
    };
    let (scope, setup, outer, nested) =
        (find("scope"), find("setup"), find("outer"), find("nested"));
    let nested_block = module.functions[scope]
        .body
        .blocks
        .iter()
        .find(|(_, block)| block.statements.iter().any(|statement| {
            matches!(&statement.kind, mir::StatementKind::Call(
                mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. }
            ) if matches!(call.target, mir::CallTarget::Direct(mir::Callee::User(target)) if target == nested))
        }))
        .unwrap()
        .0;
    let report = crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
    assert_mir_foundation_projection(&module);
    let first = report
        .events
        .iter()
        .find(|event| {
            event.site.function == scope
                && matches!(
                    event.decision,
                    OptimizationDecision::Inline {
                        outcome: InlineOutcome::Inlined,
                        ..
                    }
                )
        })
        .unwrap();
    assert_eq!(first.site.block, nested_block);
    let mut retained_setup = 0;
    for (_, block) in module.functions[scope].body.blocks.iter() {
        for statement in &block.statements {
            if let mir::StatementKind::Call(
                mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. },
            ) = &statement.kind
                && let mir::CallTarget::Direct(mir::Callee::User(target)) = call.target
            {
                assert_ne!(target, outer);
                assert_ne!(target, nested);
                retained_setup += usize::from(target == setup);
            }
        }
    }
    assert!(
        retained_setup > 0,
        "the same finite growth limit still applies"
    );
}
