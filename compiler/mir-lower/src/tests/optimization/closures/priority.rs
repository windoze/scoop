use super::*;
use mir::optimization_report::{InlineOutcome, OptimizationDecision};

fn wrapper(operations: usize, observes_identity: bool) -> (mir::Module, mir::FunctionId) {
    let mut module = sample::sample(false, 1);
    let helper = module
        .functions
        .iter()
        .find(|(_, f)| f.name == "invoke")
        .unwrap()
        .0;
    let function = &mut module.functions[helper];
    let parameter = &function.params[0];
    if observes_identity {
        let value = mir::Expr::local(parameter.local, parameter.ty.clone());
        function.body.blocks[function.body.entry].statements.insert(
            0,
            mir::Statement {
                debug_origin: Default::default(),
                span: crate::source_span(SPAN),
                kind: mir::StatementKind::Expr(mir::Expr::new(
                    mir::Type::Boolean,
                    mir::ExprKind::Binary {
                        op: mir::BinOp::RefEq,
                        lhs: Box::new(value.clone()),
                        rhs: Box::new(value),
                    },
                )),
            },
        );
    }
    for (_, block) in function.body.blocks.iter_mut() {
        if let mir::Terminator::Return { value } = &mut block.terminator {
            let input = value.clone();
            for _ in 0..operations {
                *value = mir::Expr::integer_binary(
                    mir::IntegerBinaryOperation::new(
                        mir::IntegerKind::SIGNED_32,
                        mir::IntegerBinaryOperator::Add,
                    ),
                    value.clone(),
                    input.clone(),
                );
            }
        }
    }
    (module, helper)
}

fn outcomes(
    report: &mir::optimization_report::OptimizationReport,
    helper: mir::FunctionId,
) -> Vec<InlineOutcome> {
    report
        .events
        .iter()
        .filter_map(|event| match &event.decision {
            OptimizationDecision::Inline {
                target: mir::CallTarget::Direct(mir::Callee::User(id)),
                outcome,
                estimate,
                ..
            } if *id == helper => {
                if *outcome == InlineOutcome::KnownClosure {
                    let estimate = estimate.as_ref().unwrap();
                    assert!(estimate.ordinary.growth <= estimate.remaining_growth);
                }
                Some(*outcome)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn known_synchronous_callback_exposes_and_eliminates_its_environment() {
    let (mut module, helper) = wrapper(40, false);
    let report = crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
    assert!(outcomes(&report, helper).contains(&InlineOutcome::KnownClosure));
    assert_eq!(counts(&module), (0, 0, 0));
    assert_mir_foundation_projection(&module);
}

#[test]
fn identity_observation_does_not_get_callback_priority() {
    let (mut module, helper) = wrapper(40, true);
    let report = crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
    let outcomes = outcomes(&report, helper);
    assert!(!outcomes.contains(&InlineOutcome::KnownClosure));
    assert!(outcomes.contains(&InlineOutcome::Profitability));
    assert_eq!(counts(&module).0, 1);
    assert_mir_foundation_projection(&module);
}

#[test]
fn known_callback_still_respects_caller_growth() {
    let (mut module, helper) = wrapper(300, false);
    let report = crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
    let outcomes = outcomes(&report, helper);
    assert!(!outcomes.contains(&InlineOutcome::KnownClosure));
    assert!(outcomes.contains(&InlineOutcome::CallerGrowth));
    assert_eq!(counts(&module).0, 1);
    assert_mir_foundation_projection(&module);
}
