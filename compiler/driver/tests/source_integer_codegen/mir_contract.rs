use std::collections::{HashMap, HashSet};

use super::IntegerCase;

pub(super) fn assert_contract(function: &scoop_mir::Function, case: IntegerCase) {
    assert_div_rem_guards(function, case);
    assert_shift_mask(function, case);
}

fn successors(block: &scoop_mir::BasicBlock) -> Vec<scoop_mir::BlockId> {
    let mut successors = match block.terminator {
        scoop_mir::Terminator::Goto(target) => vec![target],
        scoop_mir::Terminator::Branch {
            then_block,
            else_block,
            ..
        } => vec![then_block, else_block],
        scoop_mir::Terminator::Return { .. }
        | scoop_mir::Terminator::Throw { .. }
        | scoop_mir::Terminator::Rethrow { .. }
        | scoop_mir::Terminator::Resume
        | scoop_mir::Terminator::Trap { .. }
        | scoop_mir::Terminator::Unreachable => Vec::new(),
    };
    if let Some(unwind) = block.unwind {
        successors.push(unwind);
    }
    successors
}

fn dominators(body: &scoop_mir::Body) -> HashMap<scoop_mir::BlockId, HashSet<scoop_mir::BlockId>> {
    let mut reachable = HashSet::new();
    let mut pending = vec![body.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(successors(&body.blocks[block]));
        }
    }

    let mut predecessors = HashMap::<scoop_mir::BlockId, Vec<scoop_mir::BlockId>>::new();
    for block in reachable.iter().copied() {
        for successor in successors(&body.blocks[block]) {
            if reachable.contains(&successor) {
                predecessors.entry(successor).or_default().push(block);
            }
        }
    }

    let mut result = HashMap::new();
    for block in reachable.iter().copied() {
        result.insert(
            block,
            if block == body.entry {
                HashSet::from([block])
            } else {
                reachable.clone()
            },
        );
    }
    loop {
        let previous = result.clone();
        let mut changed = false;
        for block in reachable
            .iter()
            .copied()
            .filter(|block| *block != body.entry)
        {
            let mut incoming = predecessors
                .get(&block)
                .expect("reachable non-entry block has a predecessor")
                .iter();
            let first = *incoming.next().expect("predecessor set is non-empty");
            let mut next = previous[&first].clone();
            for predecessor in incoming {
                next.retain(|candidate| previous[predecessor].contains(candidate));
            }
            next.insert(block);
            if next != previous[&block] {
                result.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return result;
        }
    }
}

fn direct_statement_value(statement: &scoop_mir::Statement) -> Option<&scoop_mir::Expr> {
    match &statement.kind {
        scoop_mir::StatementKind::Expr(value)
        | scoop_mir::StatementKind::ValDecl { init: value, .. }
        | scoop_mir::StatementKind::Assign { value, .. }
        | scoop_mir::StatementKind::GlobalAssign { value, .. } => Some(value),
        scoop_mir::StatementKind::Call(_)
        | scoop_mir::StatementKind::ArraySet { .. }
        | scoop_mir::StatementKind::FieldSet { .. }
        | scoop_mir::StatementKind::AtomicFieldStore { .. }
        | scoop_mir::StatementKind::Eh(_) => None,
    }
}

fn assigned_value(
    block: &scoop_mir::BasicBlock,
    local: scoop_mir::LocalId,
) -> Option<&scoop_mir::Expr> {
    block
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            scoop_mir::StatementKind::Assign {
                local: destination,
                value,
            } if *destination == local => Some(value),
            _ => None,
        })
}

fn integer_equality(
    expression: &scoop_mir::Expr,
    local: scoop_mir::LocalId,
    kind: scoop_mir::IntegerKind,
    raw_bits: u64,
) -> bool {
    let scoop_mir::ExprKind::IntegerCompare {
        operation,
        lhs,
        rhs,
    } = &expression.kind
    else {
        return false;
    };
    if operation.operand_kind() != kind
        || operation.operator() != scoop_mir::IntegerComparisonOperator::Equal
    {
        return false;
    }
    let matches_pair = |local_value: &scoop_mir::Expr, constant: &scoop_mir::Expr| {
        matches!(local_value.kind, scoop_mir::ExprKind::Local(found) if found == local)
            && matches!(
                constant.kind,
                scoop_mir::ExprKind::IntegerLiteral(value)
                    if value.kind() == kind && value.raw_bits() == raw_bits
            )
    };
    matches_pair(lhs, rhs) || matches_pair(rhs, lhs)
}

#[derive(Clone, Copy)]
struct SafeSite {
    block: scoop_mir::BlockId,
    destination: scoop_mir::LocalId,
    operation: scoop_mir::SafeIntegerDivRemOperator,
    lhs: scoop_mir::LocalId,
    rhs: scoop_mir::LocalId,
}

fn safe_sites(function: &scoop_mir::Function) -> Vec<SafeSite> {
    let mut sites = Vec::new();
    for (block_id, block) in function.body.blocks.iter() {
        for statement in &block.statements {
            let destination = match statement.kind {
                scoop_mir::StatementKind::ValDecl { local, .. }
                | scoop_mir::StatementKind::Assign { local, .. } => local,
                _ => continue,
            };
            let Some(value) = direct_statement_value(statement) else {
                continue;
            };
            let scoop_mir::ExprKind::SafeIntegerDivRem {
                operation,
                lhs,
                rhs,
            } = &value.kind
            else {
                continue;
            };
            let (scoop_mir::ExprKind::Local(lhs), scoop_mir::ExprKind::Local(rhs)) =
                (&lhs.kind, &rhs.kind)
            else {
                panic!("MIR safe integer operation must consume captured locals");
            };
            sites.push(SafeSite {
                block: block_id,
                destination,
                operation: operation.operator(),
                lhs: *lhs,
                rhs: *rhs,
            });
        }
    }
    sites
}

fn assert_div_rem_guards(function: &scoop_mir::Function, case: IntegerCase) {
    let sites = safe_sites(function);
    assert_eq!(sites.len(), 2, "{} MIR safe sites", case.function);
    assert_eq!(
        sites.iter().map(|site| site.operation).collect::<Vec<_>>(),
        [
            scoop_mir::SafeIntegerDivRemOperator::Divide,
            scoop_mir::SafeIntegerDivRemOperator::Remainder,
        ],
        "{} keeps typed div/rem order",
        case.function
    );
    let dominates = dominators(&function.body);

    for site in sites {
        let zero_guards = function
            .body
            .blocks
            .iter()
            .filter_map(|(block_id, block)| match &block.terminator {
                scoop_mir::Terminator::Branch {
                    cond,
                    then_block,
                    else_block,
                } if integer_equality(cond, site.rhs, case.mir_kind, 0) => {
                    Some((block_id, *then_block, *else_block))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            zero_guards.len(),
            1,
            "{} {:?} has one exact zero guard",
            case.function,
            site.operation
        );
        let (_, zero_path, nonzero_path) = zero_guards[0];
        assert!(
            matches!(
                function.body.blocks[zero_path].terminator,
                scoop_mir::Terminator::Throw { .. }
            ),
            "{} {:?} zero path throws",
            case.function,
            site.operation
        );
        assert!(
            dominates[&site.block].contains(&nonzero_path),
            "{} {:?} is dominated by the nonzero edge",
            case.function,
            site.operation
        );

        if case.mir_kind.signedness() == scoop_mir::IntegerSignedness::Signed {
            assert_signed_boundary(function, case, site, nonzero_path, &dominates);
        }
    }
}

fn assert_signed_boundary(
    function: &scoop_mir::Function,
    case: IntegerCase,
    site: SafeSite,
    nonzero_path: scoop_mir::BlockId,
    dominates: &HashMap<scoop_mir::BlockId, HashSet<scoop_mir::BlockId>>,
) {
    let boundary_branches = function
        .body
        .blocks
        .iter()
        .filter_map(|(block_id, block)| match &block.terminator {
            scoop_mir::Terminator::Branch {
                cond,
                then_block,
                else_block,
            } if *else_block == site.block => match cond.kind {
                scoop_mir::ExprKind::Local(boundary) => Some((block_id, boundary, *then_block)),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        boundary_branches.len(),
        1,
        "{} {:?} has one MIN/-1 boundary branch",
        case.function,
        site.operation
    );
    let (boundary_block, boundary, special_path) = boundary_branches[0];
    assert!(dominates[&boundary_block].contains(&nonzero_path));

    let rhs_minus_one = blocks_assigning(function, boundary, |value| {
        integer_equality(
            value,
            site.rhs,
            case.mir_kind,
            case.mir_kind.width().raw_mask(),
        )
    });
    let boundary_false = blocks_assigning(function, boundary, |value| {
        matches!(value.kind, scoop_mir::ExprKind::BoolLiteral(false))
    });
    assert_eq!(rhs_minus_one.len(), 1);
    assert_eq!(boundary_false.len(), 1);
    for assignment in [rhs_minus_one[0], boundary_false[0]] {
        assert!(matches!(
            function.body.blocks[assignment].terminator,
            scoop_mir::Terminator::Goto(target) if target == boundary_block
        ));
    }

    let sign_bit = 1_u64 << (case.mir_kind.width().bits() - 1);
    let min_guards = function
        .body
        .blocks
        .iter()
        .filter_map(|(block_id, block)| match &block.terminator {
            scoop_mir::Terminator::Branch {
                cond,
                then_block,
                else_block,
            } if *then_block == rhs_minus_one[0]
                && *else_block == boundary_false[0]
                && integer_equality(cond, site.lhs, case.mir_kind, sign_bit) =>
            {
                Some(block_id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        min_guards.len(),
        1,
        "{} {:?} tests signed MIN after nonzero",
        case.function,
        site.operation
    );
    assert!(dominates[&min_guards[0]].contains(&nonzero_path));

    let safe_merge = match function.body.blocks[site.block].terminator {
        scoop_mir::Terminator::Goto(target) => target,
        _ => panic!("safe signed integer block must join its special path"),
    };
    assert!(matches!(
        function.body.blocks[special_path].terminator,
        scoop_mir::Terminator::Goto(target) if target == safe_merge
    ));
    let special_bits = match site.operation {
        scoop_mir::SafeIntegerDivRemOperator::Divide => sign_bit,
        scoop_mir::SafeIntegerDivRemOperator::Remainder => 0,
    };
    assert!(
        assigned_value(&function.body.blocks[special_path], site.destination).is_some_and(
            |value| matches!(
                value.kind,
                scoop_mir::ExprKind::IntegerLiteral(constant)
                    if constant.kind() == case.mir_kind && constant.raw_bits() == special_bits
            )
        ),
        "{} {:?} handles MIN/-1 without primitive LLVM division",
        case.function,
        site.operation
    );
}

fn blocks_assigning(
    function: &scoop_mir::Function,
    local: scoop_mir::LocalId,
    predicate: impl Fn(&scoop_mir::Expr) -> bool,
) -> Vec<scoop_mir::BlockId> {
    function
        .body
        .blocks
        .iter()
        .filter_map(|(block, contents)| {
            assigned_value(contents, local)
                .is_some_and(&predicate)
                .then_some(block)
        })
        .collect()
}

fn assert_shift_mask(function: &scoop_mir::Function, case: IntegerCase) {
    let shifted_returns = function
        .body
        .blocks
        .iter()
        .filter_map(|(_, block)| match &block.terminator {
            scoop_mir::Terminator::Return { value: Some(value) }
                if matches!(value.kind, scoop_mir::ExprKind::IntegerShift { .. }) =>
            {
                Some(value)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(shifted_returns.len(), 1);
    let scoop_mir::ExprKind::IntegerShift {
        operation, count, ..
    } = &shifted_returns[0].kind
    else {
        unreachable!("filtered above")
    };
    assert_eq!(operation.value_kind(), case.mir_kind);
    let scoop_mir::ExprKind::IntegerConversion {
        conversion,
        operand,
    } = &count.kind
    else {
        panic!("{} shift count must be width-normalized", case.function);
    };
    assert_eq!(conversion.source_kind(), scoop_mir::IntegerKind::SIGNED_64);
    assert_eq!(conversion.target_kind(), case.mir_kind);
    let scoop_mir::ExprKind::IntegerBinary {
        operation,
        lhs,
        rhs,
    } = &operand.kind
    else {
        panic!("{} shift normalization must include a mask", case.function);
    };
    assert_eq!(operation.kind(), scoop_mir::IntegerKind::SIGNED_64);
    assert_eq!(
        operation.operator(),
        scoop_mir::IntegerBinaryOperator::BitAnd
    );
    let expected_mask = u64::from(case.mir_kind.width().bits() - 1);
    assert!(
        [lhs.as_ref(), rhs.as_ref()].iter().any(|value| matches!(
            value.kind,
            scoop_mir::ExprKind::IntegerLiteral(constant)
                if constant.kind() == scoop_mir::IntegerKind::SIGNED_64
                    && constant.raw_bits() == expected_mask
        )),
        "{} masks its Long shift count by {expected_mask}",
        case.function
    );
}
