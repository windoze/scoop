//! Optional local observations, excluded from MIR metadata and code identity.

use crate::{BlockId, CallTarget, Callee, FunctionId, SourceSpan, Type};

#[derive(Debug, Default)]
pub struct OptimizationReport {
    pub inlining: bool,
    pub allocation_elision: bool,
    pub rounds: usize,
    pub converged: bool,
    pub events: Vec<OptimizationEvent>,
}

#[derive(Debug)]
pub struct OptimizationEvent {
    pub round: usize,
    pub site: OptimizationSite,
    pub decision: OptimizationDecision,
}

/// Coordinates refer to the body at the instant of the observation.
#[derive(Debug)]
pub struct OptimizationSite {
    pub function: FunctionId,
    pub block: BlockId,
    pub statement: usize,
    pub span: SourceSpan,
    pub inline_path: Vec<Callee>,
}

#[derive(Debug)]
pub enum OptimizationDecision {
    Inline {
        target: CallTarget<()>,
        outcome: InlineOutcome,
        estimate: Option<Box<InlineEstimate>>,
    },
    Allocation {
        kind: AllocationKind,
        ty: Type,
        outcome: AllocationOutcome,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InlineOutcome {
    Inlined,
    ClassFields,
    Required,
    Profitability,
    CallerGrowth,
    IndirectCall,
    NoBody,
    UnsupportedBody,
    RecursiveCallee,
    ProtocolCallee,
    ProtocolCaller,
    RecursiveExpansion,
    Depth,
    GrowthExhausted,
    ReleasePublication,
}

#[derive(Debug)]
pub struct InlineEstimate {
    pub original_execution: usize,
    pub original_size: usize,
    pub baseline_execution: usize,
    pub baseline_size: usize,
    pub ordinary: InlineCost,
    pub projection: InlineProjection,
    pub remaining_growth: usize,
    pub region_remaining_growth: usize,
    pub caller_remaining_growth: usize,
    pub in_loop: bool,
    pub heat: InlineHeat,
    pub caller_size: usize,
    pub constant_arguments: usize,
    pub exact_arguments: usize,
    pub result_used: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct InlineHeat {
    pub source: InlineHeatSource,
    pub loop_depth: usize,
    pub loop_header: Option<BlockId>,
    pub numerator: usize,
    pub denominator: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum InlineHeatSource {
    Ordinary,
    Loop,
    ColdExit,
}

#[derive(Clone, Copy, Debug)]
pub struct InlineBenefit {
    pub call: usize,
    pub abi: usize,
    pub simplification: usize,
    pub projection: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct InlineCost {
    pub execution: usize,
    pub size: usize,
    pub net_growth: usize,
    pub growth: usize,
    pub complexity: usize,
    pub benefit: InlineBenefit,
    pub weighted_benefit: usize,
    pub penalty: usize,
}

#[derive(Debug)]
pub enum InlineProjection {
    NotAttempted,
    Unproven,
    Estimated(InlineCost),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationKind {
    Box,
    Closure,
    Class,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationOutcome {
    Eliminated,
    ReleaseHook,
    Representation,
    AmbiguousSource,
    LoopCarried,
    NotDominating,
    UnreplacedUses { reads: usize, replacements: usize },
}
