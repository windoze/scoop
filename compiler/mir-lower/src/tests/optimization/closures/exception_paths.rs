use super::*;

fn protected() -> (mir::Module, mir::FunctionId) {
    let mut module = sample::sample(false, 1);
    let helper = module
        .functions
        .iter()
        .find(|(_, f)| f.name == "invoke")
        .unwrap()
        .0;
    let body = &mut module.functions[helper].body;
    let statement = |kind| mir::Statement {
        kind,
        span: crate::source_span(SPAN),
    };
    let caught = body.blocks.alloc(mir::BasicBlock {
        name: "caught".into(),
        statements: vec![
            statement(mir::StatementKind::Eh(mir::EhStatement::BeginCatch)),
            statement(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
        ],
        terminator: mir::Terminator::Return {
            value: mir::Expr::integer(mir::MirIntegerConstant::Signed32(99)),
        },
        unwind: None,
    });
    let pad = body.blocks.alloc(mir::BasicBlock {
        name: "landing".into(),
        statements: vec![statement(mir::StatementKind::Eh(
            mir::EhStatement::LandingPad { cleanup: false },
        ))],
        terminator: mir::Terminator::Goto(caught),
        unwind: None,
    });
    body.blocks[body.entry].unwind = Some(pad);
    (module, helper)
}

#[test]
fn exact_callback_elides_the_environment_without_rewriting_the_generic_handler() {
    let (mut module, helper) = protected();
    optimize(&mut module);
    assert_eq!(counts(&module), (0, 0, 0));
    assert!(
        module.functions[scope(&module)]
            .body
            .blocks
            .iter()
            .all(|(_, b)| {
                b.unwind.is_none()
                    && b.statements
                        .iter()
                        .all(|s| !matches!(s.kind, mir::StatementKind::Eh(_)))
            })
    );
    assert!(
        module.functions[helper]
            .body
            .blocks
            .iter()
            .any(|(_, b)| b.unwind.is_some())
    );
}

#[test]
fn disabled_inlining_preserves_callback_and_exception_paths() {
    let (mut module, helper) = protected();
    run(&mut module, MirOptimizationOptions::default(), 3);
    assert_eq!(counts(&module), (1, 0, 0));
    assert!(
        module.functions[helper]
            .body
            .blocks
            .iter()
            .any(|(_, b)| b.unwind.is_some())
    );
}
