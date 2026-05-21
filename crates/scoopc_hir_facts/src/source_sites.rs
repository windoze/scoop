//! Source-site typed contracts exported from HIR lowering.

use scoopc_ids::{CanonicalTextKey, SiteId};
use scoopc_source::SourceMapSpan;
use scoopc_types::{EffectRow, TypeId};

/// HIR facts keyed by source-level sites inside a body.
#[derive(Debug, Clone, Default)]
pub struct SourceSiteFacts {
    pub call_sites: Vec<CallSiteContract>,
    pub argument_bindings: Vec<ArgumentBindingContract>,
    pub assignments: Vec<AssignmentContract>,
    pub with_updates: Vec<WithUpdateContract>,
    pub effect_sites: Vec<EffectSiteContract>,
    pub continuation_resumes: Vec<ContinuationResumeContract>,
    pub pattern_bindings: Vec<PatternBindingContract>,
}

impl SourceSiteFacts {
    /// Return whether no source-site contracts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.call_sites.is_empty()
            && self.argument_bindings.is_empty()
            && self.assignments.is_empty()
            && self.with_updates.is_empty()
            && self.effect_sites.is_empty()
            && self.continuation_resumes.is_empty()
            && self.pattern_bindings.is_empty()
    }
}

/// Stable source-site identity scoped to a lowered body.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceSiteIdentity {
    pub owner: CanonicalTextKey,
    pub site: SiteId,
    pub source: SourceMapSpan,
}

impl SourceSiteIdentity {
    /// Create a source-site identity from its owner, local site id, and source span.
    pub fn new(owner: CanonicalTextKey, site: SiteId, source: SourceMapSpan) -> Self {
        Self {
            owner,
            site,
            source,
        }
    }
}

/// Source-level category of a resolved call expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallSiteKind {
    Direct,
    Member,
    VirtualDispatch,
    InterfaceDispatch,
    Constructor,
    Closure,
    FunctionPointer,
    Intrinsic,
    EffectOperation,
}

/// Typed contract for a resolved call site.
#[derive(Debug, Clone)]
pub struct CallSiteContract {
    pub identity: SourceSiteIdentity,
    pub kind: CallSiteKind,
    pub resolved_target: Option<CanonicalTextKey>,
    pub receiver_ty: Option<TypeId>,
    pub argument_tys: Vec<TypeId>,
    pub return_ty: TypeId,
    pub effects: EffectRow,
}

/// Canonical mapping from source argument position to callable parameter slot.
#[derive(Debug, Clone)]
pub struct ArgumentBindingContract {
    pub identity: SourceSiteIdentity,
    pub argument_index: u32,
    pub parameter_index: u32,
    pub argument_ty: TypeId,
}

/// Typed contract for an assignment place.
#[derive(Debug, Clone)]
pub struct AssignmentContract {
    pub identity: SourceSiteIdentity,
    pub place_ty: TypeId,
    pub value_ty: TypeId,
}

/// Typed contract for aggregate copy/update syntax.
#[derive(Debug, Clone)]
pub struct WithUpdateContract {
    pub identity: SourceSiteIdentity,
    pub base_ty: TypeId,
    pub result_ty: TypeId,
    pub updated_fields: Vec<String>,
}

/// Source-level effect operation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectSiteKind {
    Perform,
    Handle,
}

/// Typed contract for `perform` and `handle` sites.
#[derive(Debug, Clone)]
pub struct EffectSiteContract {
    pub identity: SourceSiteIdentity,
    pub kind: EffectSiteKind,
    pub effect_ty: TypeId,
    pub payload_ty: Option<TypeId>,
    pub result_ty: TypeId,
}

/// Typed contract for a continuation resume site.
#[derive(Debug, Clone)]
pub struct ContinuationResumeContract {
    pub identity: SourceSiteIdentity,
    pub continuation_ty: TypeId,
    pub payload_ty: TypeId,
    pub result_ty: TypeId,
    pub resumes_outward: bool,
}

/// Precise type assigned to a source pattern binding.
#[derive(Debug, Clone)]
pub struct PatternBindingContract {
    pub identity: SourceSiteIdentity,
    pub binding_name: String,
    pub binding_ty: TypeId,
}
