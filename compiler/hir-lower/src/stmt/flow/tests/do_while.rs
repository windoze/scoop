use super::*;

#[test]
fn body_exit_precedes_condition_setup_and_continue_reaches_it() {
    let target = hir::LoopId::from_raw(0);
    for (body, expected) in [
        (vec![return_statement()], vec![ControlOutcome::Return]),
        (
            vec![statement(hir::StatementKind::Continue { target })],
            vec![ControlOutcome::Throw],
        ),
        (
            vec![statement(hir::StatementKind::Break { target })],
            vec![ControlOutcome::Fallthrough],
        ),
    ] {
        let loop_ = statement(hir::StatementKind::While {
            loop_kind: hir::LoopKind::DoWhile,
            target,
            body,
            condition_setup: vec![throw_statement()],
            cond: expression(),
        });
        assert_hir_outcomes(&[loop_], expected);
    }
}
