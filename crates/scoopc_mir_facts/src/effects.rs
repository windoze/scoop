//! MIR-published effect rows, site inventories, and effect event streams.

use scoopc_ids::{BodyBlockId, CanonicalTextKey, SiteId, StageArtifactKey};
use scoopc_span::Span;
use scoopc_types::TypeId;

use crate::common::{FactIdentity, MirBodyReference};

/// MIR-owned effect facts published before P4 effect solving.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MirEffectFacts {
    pub callable_instances: Vec<CallableInstanceEffectFacts>,
    pub site_inventory: Vec<MirSiteInventoryFact>,
    pub effect_events: Vec<MirEffectEventFact>,
    pub block_regions: Vec<MirBlockEffectRegionFact>,
    pub call_site_targets: Vec<CallSiteTargetFact>,
    pub call_site_surface_effects: Vec<CallSiteSurfaceEffectFact>,
}

impl MirEffectFacts {
    /// Return whether no MIR effect facts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.callable_instances.is_empty()
            && self.site_inventory.is_empty()
            && self.effect_events.is_empty()
            && self.block_regions.is_empty()
            && self.call_site_targets.is_empty()
            && self.call_site_surface_effects.is_empty()
    }
}

/// Stable, data-only effect row template used by MIR facts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EffectRowTemplate {
    pub terms: Vec<EffectRowTerm>,
    pub closed: bool,
}

impl EffectRowTemplate {
    /// Create a normalized effect row template.
    pub fn new(mut terms: Vec<EffectRowTerm>, closed: bool) -> Self {
        terms.sort_by_key(EffectRowTerm::canonical_text);
        terms.dedup_by(|left, right| left.canonical_text() == right.canonical_text());
        Self { terms, closed }
    }

    /// Return the open empty row.
    pub fn pure() -> Self {
        Self::new(Vec::new(), false)
    }

    /// Render a deterministic compact form for dumps and diagnostics.
    pub fn canonical_text(&self) -> String {
        let mut text = format!(
            "E({})",
            self.terms
                .iter()
                .map(EffectRowTerm::canonical_text)
                .collect::<Vec<_>>()
                .join(",")
        );
        if self.closed {
            text.push('!');
        }
        text
    }
}

/// One term in a stable effect row template.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EffectRowTerm {
    Concrete {
        type_key: CanonicalTextKey,
    },
    Param {
        owner: CanonicalTextKey,
        ordinal: u32,
        name: String,
    },
}

impl EffectRowTerm {
    /// Render this term's canonical identity.
    pub fn canonical_text(&self) -> String {
        match self {
            Self::Concrete { type_key } => type_key.as_str().to_string(),
            Self::Param {
                owner,
                ordinal,
                name,
            } => format!("eff_param({},{},{})", owner.as_str(), ordinal, name),
        }
    }
}

/// Instance-level layered effect rows after MIR materialization.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallableInstanceEffectFacts {
    pub identity: FactIdentity,
    pub instance: StageArtifactKey,
    pub callable: CanonicalTextKey,
    pub declared_surface_row: Option<EffectRowTemplate>,
    pub actual_surface_row: EffectRowTemplate,
    pub published_surface_row: EffectRowTemplate,
    pub step_effect_row: EffectRowTemplate,
}

impl CallableInstanceEffectFacts {
    /// Create layered effect facts for one materialized callable instance.
    pub fn new(
        identity: FactIdentity,
        instance: StageArtifactKey,
        callable: CanonicalTextKey,
        declared_surface_row: Option<EffectRowTemplate>,
        actual_surface_row: EffectRowTemplate,
        published_surface_row: EffectRowTemplate,
        step_effect_row: EffectRowTemplate,
    ) -> Self {
        Self {
            identity,
            instance,
            callable,
            declared_surface_row,
            actual_surface_row,
            published_surface_row,
            step_effect_row,
        }
    }
}

/// MIR site category published without exposing MIR node enums.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MirSiteKind {
    Call,
    ClassCtor,
    HiddenInitializer,
    Perform,
    Resume,
    Handle,
}

impl MirSiteKind {
    /// Return a stable dump/test label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::ClassCtor => "class_ctor",
            Self::HiddenInitializer => "hidden_initializer",
            Self::Perform => "perform",
            Self::Resume => "resume",
            Self::Handle => "handle",
        }
    }
}

/// Source inventory for one effect-relevant MIR site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MirSiteInventoryFact {
    pub identity: FactIdentity,
    pub instance: StageArtifactKey,
    pub body: MirBodyReference,
    pub site_id: SiteId,
    pub kind: MirSiteKind,
    pub block: BodyBlockId,
    pub statement_index: Option<u32>,
    pub result_local: Option<u32>,
    pub result_ty: Option<TypeId>,
    pub span: Option<Span>,
    pub cleanup: bool,
}

/// One structured effect event emitted by a MIR site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MirEffectEventFact {
    pub identity: FactIdentity,
    pub instance: StageArtifactKey,
    pub body: MirBodyReference,
    pub site_id: SiteId,
    pub kind: MirEffectEventKind,
    pub block: BodyBlockId,
    pub statement_index: Option<u32>,
    pub effect_row: EffectRowTemplate,
    pub cleanup: bool,
}

/// Stable event family emitted by MIR effect sites.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MirEffectEventKind {
    Call {
        call_kind: MirCallKind,
    },
    ClassCtor {
        source_fqn: String,
    },
    HiddenInitializer {
        source_fqn: String,
    },
    Perform {
        op: MirEffectOpSiteContract,
    },
    Resume {
        resume_tuple_ty: TypeId,
        answer_ty: TypeId,
        continuation_ty: TypeId,
        surface_row: EffectRowTemplate,
    },
    Handle {
        result_ty: TypeId,
        body_target: BodyBlockId,
        arm_targets: Vec<BodyBlockId>,
        finally_target: Option<BodyBlockId>,
        exit_target: BodyBlockId,
        arms: Vec<MirEffectOpSiteContract>,
    },
}

/// Typed effect-operation contract for one perform or handle arm site.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct MirEffectOpSiteContract {
    pub op_fqn: String,
    pub effect_ty: TypeId,
    pub op_type_args: Vec<TypeId>,
    pub payload_tuple_ty: TypeId,
}

/// Language-level call shape used by MIR facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MirCallKind {
    Direct,
    Closure,
    FunValue,
    FunPtr,
    Virtual,
    Interface,
}

impl MirCallKind {
    /// Return a stable dump/test label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Closure => "closure",
            Self::FunValue => "fun_value",
            Self::FunPtr => "fun_ptr",
            Self::Virtual => "virtual",
            Self::Interface => "interface",
        }
    }
}

/// Block-level region/inventory fact for effect solving.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MirBlockEffectRegionFact {
    pub identity: FactIdentity,
    pub instance: StageArtifactKey,
    pub body: MirBodyReference,
    pub block: BodyBlockId,
    pub site_ids: Vec<SiteId>,
    pub successors: Vec<BodyBlockId>,
    pub cleanup: bool,
    pub cleanup_target: Option<BodyBlockId>,
    pub has_suspend_boundary: bool,
    pub has_handle_boundary: bool,
}

/// Authoritative target fact for one MIR call site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallSiteTargetFact {
    pub identity: FactIdentity,
    pub instance: StageArtifactKey,
    pub body: MirBodyReference,
    pub site_id: SiteId,
    pub call_kind: MirCallKind,
    pub target: CallSiteTarget,
}

/// Stable call-site target identity.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CallSiteTarget {
    KnownInstance { key: CanonicalTextKey },
    CandidateSet { keys: Vec<CanonicalTextKey> },
    DirectFunction { fqn: String },
    KnownClosure { fn_ptr: String },
    Dynamic,
}

/// Published surface row for one MIR call site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallSiteSurfaceEffectFact {
    pub identity: FactIdentity,
    pub instance: StageArtifactKey,
    pub body: MirBodyReference,
    pub site_id: SiteId,
    pub surface_row: EffectRowTemplate,
}
