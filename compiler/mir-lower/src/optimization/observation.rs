//! Collect decisions only for an explicitly requested local report.

use std::cell::{Cell, RefCell};

use super::mir;
use mir::optimization_report::{
    OptimizationDecision, OptimizationEvent, OptimizationReport, OptimizationSite,
};

pub(super) struct Observer<'a> {
    report: RefCell<&'a mut OptimizationReport>,
    round: Cell<usize>,
}

impl<'a> Observer<'a> {
    pub fn new(report: &'a mut OptimizationReport, options: super::MirOptimizationOptions) -> Self {
        *report = OptimizationReport {
            inlining: options.inline,
            allocation_elision: options.allocation_elision,
            ..Default::default()
        };
        Self {
            report: RefCell::new(report),
            round: Cell::new(0),
        }
    }

    pub fn round(&self, round: usize) {
        self.round.set(round);
    }

    pub fn finish(&self, (rounds, converged): (usize, bool)) {
        let mut report = self.report.borrow_mut();
        report.rounds = rounds;
        report.converged = converged;
    }

    pub fn record(&self, site: OptimizationSite, decision: OptimizationDecision) {
        self.report.borrow_mut().events.push(OptimizationEvent {
            round: self.round.get(),
            site,
            decision,
        });
    }
}

pub(super) fn call_target(target: &mir::CallTarget) -> mir::CallTarget<()> {
    match *target {
        mir::CallTarget::Direct(callee) => mir::CallTarget::Direct(callee),
        mir::CallTarget::Virtual { callee, slot, .. } => mir::CallTarget::Virtual {
            callee,
            slot,
            receiver: Box::new(()),
        },
        mir::CallTarget::Interface {
            callee,
            interface,
            slot,
            ..
        } => mir::CallTarget::Interface {
            callee,
            interface,
            slot,
            receiver: Box::new(()),
        },
        mir::CallTarget::Closure { function_type, .. } => mir::CallTarget::Closure {
            function_type,
            receiver: Box::new(()),
        },
        mir::CallTarget::FunctionBridge { function_type, .. } => mir::CallTarget::FunctionBridge {
            function_type,
            receiver: Box::new(()),
        },
    }
}
