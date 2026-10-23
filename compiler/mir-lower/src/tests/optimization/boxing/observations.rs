use super::*;
use mir::optimization_report::{AllocationKind, AllocationOutcome, OptimizationDecision};

pub(super) fn observe(
    module: &mut mir::Module,
    function: mir::FunctionId,
    expected: AllocationOutcome,
) {
    let report = crate::optimization::run_observed(
        module,
        MirOptimizationOptions {
            allocation_elision: true,
            ..Default::default()
        },
    );
    let event = report
        .events
        .iter()
        .find(|event| {
            event.site.function == function
                && matches!(
                    event.decision,
                    OptimizationDecision::Allocation {
                        kind: AllocationKind::Box,
                        ..
                    }
                )
        })
        .unwrap();
    let OptimizationDecision::Allocation { outcome, .. } = event.decision else {
        unreachable!()
    };
    if matches!(expected, AllocationOutcome::UnreplacedUses { .. }) {
        assert!(
            matches!(outcome, AllocationOutcome::UnreplacedUses { reads, replacements } if reads > replacements)
        );
    } else {
        assert_eq!(outcome, expected);
    }
}

#[test]
fn allocation_reports_elision_escape_mixed_origin_and_exception_flow() {
    for case in 0..4 {
        let (mut module, function) = sample();
        let body = &mut module.functions[function].body;
        let entry = body.entry;
        let expected = match case {
            0 => AllocationOutcome::Eliminated,
            1 => {
                let observed = node(mir::StatementKind::Expr(mir::Expr::new(
                    mir::Type::Boolean,
                    mir::ExprKind::Binary {
                        op: mir::BinOp::RefEq,
                        lhs: Box::new(value(body, "alias")),
                        rhs: Box::new(value(body, "external")),
                    },
                )));
                body.blocks[entry].statements.push(observed);
                AllocationOutcome::UnreplacedUses {
                    reads: 0,
                    replacements: 0,
                }
            }
            2 => {
                let mixed = node(mir::StatementKind::Assign {
                    local: named(body, "alias"),
                    value: value(body, "external"),
                });
                body.blocks[entry].statements.insert(3, mixed);
                AllocationOutcome::AmbiguousSource
            }
            _ => {
                let landing = body.blocks.alloc(mir::BasicBlock {
                    name: "landing".into(),
                    statements: Vec::new(),
                    terminator: mir::Terminator::Unreachable,
                    unwind: None,
                });
                body.blocks[entry].unwind = Some(landing);
                AllocationOutcome::ExceptionFlow
            }
        };
        observe(&mut module, function, expected);
        assert_eq!(
            counts(&module.functions[function]).0,
            usize::from(case != 0)
        );
    }
}
