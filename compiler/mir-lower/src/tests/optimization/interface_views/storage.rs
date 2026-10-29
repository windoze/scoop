use super::*;

#[test]
fn an_overwritten_source_does_not_change_its_old_snapshot() {
    let (mut module, scope) = sample(Source::Child);
    let function = &mut module.functions[scope];
    let value = function.params[0].local;
    let replacement = function.params[1].local;
    function.body.blocks[function.body.entry].statements.insert(
        1,
        mir::Statement {
            debug_origin: Default::default(),
            kind: mir::StatementKind::Assign {
                local: value,
                value: mir::Expr::local(replacement, function.params[1].ty.clone()),
            },
            span: crate::source_span(SPAN),
        },
    );
    reuse(&mut module);
    let converted = conversions(&module.functions[scope].body);
    assert_eq!(converted.len(), 2);
    let sources = view_sources(&module.functions[scope].body);
    assert_ne!(sources[0], sources[1]);
}

#[test]
fn addressed_storage_is_not_used_as_a_stable_view_source() {
    let (mut module, scope) = sample(Source::Child);
    let function = &mut module.functions[scope];
    let value = function.params[0].local;
    let ty = function.params[0].ty.clone();
    function.body.blocks[function.body.entry].statements.insert(
        1,
        mir::Statement {
            debug_origin: Default::default(),
            kind: mir::StatementKind::Expr(mir::Expr::new(
                mir::Type::Ptr(Box::new(ty.clone())),
                mir::ExprKind::AddressOf {
                    local: value,
                    pointee: Box::new(ty),
                },
            )),
            span: crate::source_span(SPAN),
        },
    );
    reuse(&mut module);
    assert_eq!(conversions(&module.functions[scope].body).len(), 2);
}

#[test]
fn views_do_not_cross_ordinary_loop_or_exception_entries() {
    for edge in ["forward", "loop", "exception"] {
        let (mut module, scope) = sample(Source::Child);
        let body = &mut module.functions[scope].body;
        let entry = body.entry;
        let second = body.blocks[entry].statements.pop().unwrap();
        assert!(matches!(second.kind, mir::StatementKind::Call(_)));
        let next = body.blocks.alloc(mir::BasicBlock {
            terminator_origin: Default::default(),
            name: edge.to_owned(),
            statements: vec![second],
            terminator: if edge == "loop" {
                mir::Terminator::Goto(entry)
            } else {
                body.blocks[entry].terminator.clone()
            },
            unwind: None,
        });
        if edge == "exception" {
            body.blocks[entry].unwind = Some(next);
        } else {
            body.blocks[entry].terminator = mir::Terminator::Goto(next);
        }
        reuse(&mut module);
        assert_eq!(
            conversions(&module.functions[scope].body).len(),
            2,
            "{edge}"
        );
    }
}
