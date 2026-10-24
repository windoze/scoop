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
        estimate: Option<InlineEstimate>,
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
    ExecutionCost,
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
    pub ordinary: InlineCost,
    pub projection: InlineProjection,
    pub remaining_growth: usize,
    pub in_loop: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct InlineCost {
    pub execution: usize,
    pub size: usize,
    pub growth: usize,
    /// None when an earlier growth check or mandatory expansion bypassed it.
    pub execution_limit: Option<usize>,
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
