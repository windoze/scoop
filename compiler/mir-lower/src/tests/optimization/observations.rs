use super::*;
use mir::optimization_report::{InlineOutcome, OptimizationDecision};

#[test]
fn retained_call_in_an_inlined_body_keeps_its_source_path_and_span() {
    let mut h = Harness::new();
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", h.int));
    let recursive = h.user_fn_full(
        "recursive",
        Vec::new(),
        vec![param("input", h.int, input)],
        h.int,
        hir::Body {
            locals,
            statements: Vec::new(),
        },
    );
    let recursion = stmt(hir::StatementKind::Return {
        value: call_typed(recursive, vec![local_ref(input, h.int)], h.int),
    });
    let hir::FunctionKind::User(body) = &mut h.functions[recursive].kind else {
        unreachable!()
    };
    body.statements.push(recursion);
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", h.int));
    let wrapper = h.user_fn_full(
        "wrapper",
        Vec::new(),
        vec![param("input", h.int, input)],
        h.int,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: call_typed(recursive, vec![local_ref(input, h.int)], h.int),
            })],
        },
    );
    let call = expr_stmt(call_typed(wrapper, vec![int_lit(&h, 17)], h.int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![call],
        },
    );
    let mut module = lower(&h.finish(main));
    let main = module.executable_entry().unwrap();
    let wrapper = module
        .functions
        .iter()
        .find(|(_, f)| f.name == "wrapper")
        .unwrap()
        .0;
    let source_span = mir::SourceSpan::new(17, 29).unwrap();
    let source_origin = mir::DebugRecords::default().source(
        std::sync::Arc::new(mir::DebugFile {
            identity: std::sync::Arc::new(scoop_identity::SourceIdentity::single_file()),
            path: "/workspace/main.scoop".into(),
        }),
        4,
        7,
    );
    for (_, block) in module.functions[wrapper].body.blocks.iter_mut() {
        for statement in &mut block.statements {
            if matches!(statement.kind, mir::StatementKind::Call(_)) {
                statement.span = source_span;
                statement.debug_origin = source_origin.clone();
            }
        }
    }
    let report = crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
    let retained = module.functions[main]
        .body
        .blocks
        .iter()
        .flat_map(|(_, block)| &block.statements)
        .find(|statement| {
            statement.span == source_span && matches!(statement.kind, mir::StatementKind::Call(_))
        })
        .expect("recursive call remains in the inlined wrapper");
    let mir::DebugOrigin::Source(location) = &retained.debug_origin else {
        panic!("inlining retains the callee source location");
    };
    assert_eq!((location.line, location.column), (4, 7));
    assert_eq!(
        location.inlined_at.as_ref().unwrap().function.as_ref(),
        "wrapper"
    );
    assert!(report.events.iter().any(|event| event.site.function == main
        && event.site.inline_path == [mir::Callee::User(main), mir::Callee::User(wrapper)]
        && event.site.span == source_span
        && matches!(
            event.decision,
            OptimizationDecision::Inline {
                outcome: InlineOutcome::RecursiveCallee,
                ..
            }
        )));
}
