use super::*;
use la_arena::Arena;

#[test]
fn natural_loop_depth_merges_latches_and_accounts_for_unwind_nodes() {
    let mut blocks = Arena::new();
    let ids = (0..7)
        .map(|id| {
            blocks.alloc(mir::BasicBlock {
                name: format!("b{id}"),
                statements: Vec::new(),
                terminator: mir::Terminator::Unreachable,
                unwind: None,
            })
        })
        .collect::<Vec<_>>();
    let branch = |then_block, else_block| mir::Terminator::Branch {
        cond: mir::Expr::new(mir::Type::Boolean, mir::ExprKind::BoolLiteral(true)),
        then_block,
        else_block,
    };
    blocks[ids[0]].terminator = mir::Terminator::Goto(ids[1]);
    blocks[ids[1]].terminator = branch(ids[2], ids[6]);
    blocks[ids[2]].terminator = branch(ids[3], ids[5]);
    blocks[ids[3]].terminator = branch(ids[2], ids[4]);
    blocks[ids[4]].terminator = mir::Terminator::Goto(ids[2]);
    blocks[ids[4]].unwind = Some(ids[6]);
    blocks[ids[5]].terminator = mir::Terminator::Goto(ids[1]);
    let body = mir::Body {
        entry: ids[0],
        blocks,
        locals: Arena::new(),
        loop_header_polls: Vec::new(),
    };
    let nesting = ControlFlow::new(&body).loop_nesting();
    assert_eq!(
        nesting.iter().map(|n| n.depth).collect::<Vec<_>>(),
        [0, 1, 2, 2, 2, 1, 0]
    );
    assert_eq!(
        nesting
            .iter()
            .map(|n| n.header.map(index))
            .collect::<Vec<_>>(),
        [None, Some(1), Some(2), Some(2), Some(2), Some(1), None]
    );
}
