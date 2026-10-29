use super::*;
use mir::optimization_report::{AllocationKind, AllocationOutcome, OptimizationDecision};

fn handler(body: &mut mir::Body, value: mir::Expr) -> mir::BlockId {
    body.blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "handler".into(),
        statements: vec![
            node(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: false,
            })),
            node(mir::StatementKind::Eh(mir::EhStatement::BeginCatch)),
            node(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
        ],
        terminator: mir::Terminator::Return { value },
        unwind: None,
    })
}

fn observe(module: &mut mir::Module, function: mir::FunctionId, expected: AllocationOutcome) {
    let report = crate::optimization::run_observed(
        module,
        MirOptimizationOptions {
            allocation_elision: true,
            ..Default::default()
        },
    );
    let outcome = report.events.iter().find_map(|event| {
        if event.site.function == function
            && let OptimizationDecision::Allocation {
                kind: AllocationKind::Class,
                outcome,
                ..
            } = event.decision
        {
            Some(outcome)
        } else {
            None
        }
    });
    assert_eq!(outcome, Some(expected));
    assert_mir_foundation_projection(module);
}

#[test]
fn initialized_fields_are_replaced_in_both_normal_and_exception_paths() {
    let (mut module, id) = sample(false);
    let body = &mut module.functions[id].body;
    let entry = body.entry;
    let current = read(value(body, "object"), 1, value(body, "input").ty);
    let handler = handler(body, current);
    let updates = body.blocks[entry].statements.split_off(4);
    let protected = body.blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "protected".into(),
        statements: updates,
        terminator: body.blocks[entry].terminator.clone(),
        unwind: Some(handler),
    });
    body.blocks[entry].terminator = mir::Terminator::Goto(protected);
    observe(&mut module, id, AllocationOutcome::Eliminated);
    let function = &module.functions[id];
    assert_eq!(counts(function), (0, 0, 0));
    let handler = function
        .body
        .blocks
        .iter()
        .find(|(_, b)| b.name == "handler")
        .unwrap()
        .1;
    assert_eq!(handler.statements.len(), 3);
    let mir::Terminator::Return { value } = &handler.terminator else {
        unreachable!()
    };
    let mir::ExprKind::Local(local) = value.kind else {
        unreachable!()
    };
    assert!(function.body.locals[local].name.starts_with("$allocation."));
    assert!(function.body.locals[local].mutable);
}

#[test]
fn a_creation_in_a_protected_block_does_not_dominate_its_early_unwind() {
    let (mut module, id) = sample(false);
    let body = &mut module.functions[id].body;
    let current = read(value(body, "object"), 1, value(body, "input").ty);
    let handler = handler(body, current);
    body.blocks[body.entry].unwind = Some(handler);
    observe(&mut module, id, AllocationOutcome::NotDominating);
    assert_eq!(counts(&module.functions[id]).0, 1);
}

#[test]
fn later_alias_definitions_do_not_hide_the_previous_iteration_from_a_handler() {
    let (mut module, id) = sample(false);
    let body = &mut module.functions[id].body;
    let entry = body.entry;
    let previous = read(value(body, "alias"), 1, value(body, "input").ty);
    let handler = handler(body, previous);
    let alias = body.blocks[entry].statements.remove(3);
    body.blocks[entry].statements.truncate(3);
    let update = body.blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "update.alias".into(),
        statements: vec![alias],
        terminator: mir::Terminator::Goto(entry),
        unwind: Some(handler),
    });
    body.blocks[entry].terminator = mir::Terminator::Goto(update);
    observe(&mut module, id, AllocationOutcome::LoopCarried);
    assert_eq!(counts(&module.functions[id]).0, 1);
}
