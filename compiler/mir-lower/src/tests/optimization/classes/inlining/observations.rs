use super::*;
use mir::optimization_report::{
    AllocationOutcome, InlineOutcome, InlineProjection, OptimizationDecision,
};

#[test]
fn report_observes_real_projection_cost_without_changing_mir() {
    for enabled in [false, true] {
        let (mut plain, _, _) = program();
        run(&mut plain, enabled);
        let (mut observed, scope, method) = program();
        let report = crate::optimization::run_observed(
            &mut observed,
            MirOptimizationOptions {
                allocation_elision: enabled,
                ..MirOptimizationOptions::RELEASE
            },
        );
        assert_eq!(mir::dump(&observed), mir::dump(&plain));
        assert_mir_foundation_projection(&observed);
        let event = report
            .events
            .iter()
            .rev()
            .find(|event| {
                event.site.function == scope
                    && matches!(&event.decision, OptimizationDecision::Inline {
                target: mir::CallTarget::Direct(mir::Callee::User(id)), ..
            } if *id == method)
            })
            .unwrap();
        let OptimizationDecision::Inline {
            outcome,
            estimate: Some(estimate),
            ..
        } = &event.decision
        else {
            panic!("the receiver call was assessed");
        };
        assert!(estimate.ordinary.penalty > estimate.ordinary.weighted_benefit);
        if enabled {
            assert_eq!(*outcome, InlineOutcome::ClassFields);
            let InlineProjection::Estimated(cost) = estimate.projection else {
                panic!("projection cost")
            };
            assert!(cost.penalty <= cost.weighted_benefit);
            assert!(cost.growth <= estimate.remaining_growth);
            assert!(
                report
                    .events
                    .iter()
                    .any(|event| event.site.function == scope
                        && matches!(
                            event.decision,
                            OptimizationDecision::Allocation {
                                outcome: AllocationOutcome::Eliminated,
                                ..
                            }
                        ))
            );
        } else {
            assert_eq!(*outcome, InlineOutcome::Profitability);
            assert!(matches!(
                estimate.projection,
                InlineProjection::NotAttempted
            ));
        }
        assert!(report.events.iter().all(|event| event.round > 0
            && event.round <= report.rounds
            && event.site.inline_path.first() == Some(&mir::Callee::User(event.site.function))));
    }
}

#[test]
fn disabled_passes_have_no_decision_events() {
    let (mut module, _, _) = program();
    let before = mir::dump(&module);
    let report = crate::optimization::run_observed(&mut module, Default::default());
    assert_eq!(mir::dump(&module), before);
    assert!(!report.inlining && !report.allocation_elision);
    assert_eq!(report.rounds, 0);
    assert!(report.converged && report.events.is_empty());
}
