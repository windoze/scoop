//! Compare removable boundary work with the cost of this expansion.

use super::Cost;
use crate::optimization::inline::summary::Summary;
use scoop_mir::optimization_report::{InlineBenefit, InlineCost, InlineHeat};

pub(in crate::optimization::inline) struct Growth {
    base: usize,
    spent: usize,
}

impl Growth {
    pub(in crate::optimization::inline) fn new(size: usize, base: usize) -> Self {
        Self {
            base: (size / 2 + base).min(384),
            spent: 0,
        }
    }

    pub(in crate::optimization::inline) fn remaining(&self, heat: InlineHeat) -> usize {
        let factor = if heat.numerator >= 16 * heat.denominator {
            4
        } else if heat.numerator >= 4 * heat.denominator {
            2
        } else {
            1
        };
        (self.base * factor).saturating_sub(self.spent)
    }

    pub(in crate::optimization::inline) fn charge(&mut self, growth: usize) {
        self.spent += growth;
    }
}

pub(in crate::optimization::inline) struct Site {
    pub heat: InlineHeat,
    pub caller_size: usize,
    pub result_used: bool,
}

pub(in crate::optimization::inline) fn assess(
    summary: &Summary,
    cost: Cost,
    complexity: usize,
    simplification: usize,
    projection: usize,
    site: &Site,
) -> InlineCost {
    // No credit for GC roots, entry polls or for a dispatch already resolved.
    let benefit = InlineBenefit {
        call: 6,
        abi: summary.parameter_copies * 2
            + summary.return_copy * if site.result_used { 2 } else { 1 },
        simplification,
        projection,
    };
    let net_growth = cost.size.saturating_sub(6);
    let hot_growth = cost.execution.saturating_sub(6).min(net_growth);
    // Cold code still occupies space and consumes the full growth allowance.
    let size_penalty = (net_growth + 3 * hot_growth).div_ceil(4);
    let penalty = size_penalty + 2 * complexity + net_growth * site.caller_size / 512;
    let weighted_benefit =
        (benefit.call + benefit.abi + benefit.simplification + benefit.projection)
            * 4
            * site.heat.numerator
            / site.heat.denominator;
    InlineCost {
        execution: cost.execution,
        size: cost.size,
        net_growth,
        growth: net_growth.max(1),
        complexity,
        benefit,
        weighted_benefit,
        penalty,
    }
}
