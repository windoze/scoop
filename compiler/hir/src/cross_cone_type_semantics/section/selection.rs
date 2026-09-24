use super::*;
use scoop_identity::{ConeIdentity, CoreBuiltinNominal, PersistentExactTypeId};
use scoop_wire::{BudgetMeter, WirePath};

mod closure;
mod errors;
mod targets;
pub use errors::*;

/// Source provenance is independently reconstructed from actual committed HIR.
/// It is never stored in, or recovered from, the persistent selected records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeSectionCommittedRootOriginV1 {
    Source,
    ProtectedDefault {
        provider: ConeIdentity,
        key: ProtectedDefaultTemplateKeyV1,
    },
    PublicDefault {
        provider: ConeIdentity,
        key: ExportDefaultTemplateKeyV1,
    },
    LocalSemanticSupport {
        parent: PersistentExactTypeId,
    },
}

/// Every callback receives a target already tied to a complete terminal section
/// and the actual current-source context. A successful callback must replay the
/// source lookup/access, receiver, definition/evaluation origin, and capability
/// gate from its retained typed evidence. A candidate selected DTO is not such
/// evidence. Edges come from the complete semantic parent, not the selected list.
pub trait CommittedTypeUseSemanticAuthorityV1<E> {
    type Root;
    type Edge;
    fn committed_roots(&self) -> Result<&[Self::Root], E>;
    fn root_request(&self, root: &Self::Root) -> Result<SelectedExternalTypeUseV1, E>;
    fn root_origin(&self, root: &Self::Root) -> Result<TypeSectionCommittedRootOriginV1, E>;
    fn validate_root(
        &self,
        root: &Self::Root,
        target: CheckedTypeSelectionTargetV1<'_>,
        context: TypeSectionUseContextV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), E>;
    fn semantic_edges(&self, parent: CheckedTypeSelectionTargetV1<'_>) -> Result<&[Self::Edge], E>;
    fn edge_request(&self, edge: &Self::Edge) -> Result<SelectedExternalTypeUseV1, E>;
    /// Recursive default expansion has the same provenance/profile gate as a
    /// direct committed root. An incoming edge cannot erase a template owner.
    fn edge_origin(&self, edge: &Self::Edge) -> Result<TypeSectionCommittedRootOriginV1, E>;
    fn validate_edge(
        &self,
        parent: CheckedTypeSelectionTargetV1<'_>,
        edge: &Self::Edge,
        target: CheckedTypeSelectionTargetV1<'_>,
        context: TypeSectionUseContextV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), E>;
}

/// Read-only local proofs supplied to a committed source-use replayer.
#[derive(Clone, Copy)]
pub struct TypeSectionUseContextV1<'a> {
    pub(super) exports: &'a exports::CheckedTypeSectionExportsV1<'a>,
}
impl<'a> TypeSectionUseContextV1<'a> {
    pub const fn provider(self) -> ConeIdentity {
        self.exports.provider
    }
    pub fn graph(self) -> &'a CheckedNominalInheritanceGraphV1<'a> {
        &self.exports.graph
    }
    pub fn sources(self) -> &'a CheckedProtectedSourceInterfacesV1<'a> {
        &self.exports.sources
    }
    pub fn defaults(self) -> &'a CheckedProtectedDefaultTemplatesV1<'a> {
        &self.exports.defaults
    }
    pub fn public(self) -> CheckedTypeSectionPublicSupportV1<'a> {
        self.exports.public
    }
}

/// Presence and kind-specific identity/owner/receiver joins, not source access.
/// Source replay must succeed before this becomes a committed selected use.
#[derive(Clone, Copy, Debug)]
pub struct CheckedTypeSelectionTargetV1<'a> {
    pub(super) request: SelectedExternalTypeUseV1,
    pub(super) definition: CheckedTypeSelectionDefinitionV1<'a>,
    pub(super) facts: CheckedExactTypeFactV1<'a>,
    pub(super) public: CheckedTypeSectionPublicSupportV1<'a>,
    pub(super) declarations: CheckedProtectedDeclarationSourcesV1<'a>,
}

/// Language builtins have complete facts but no source representation or
/// inheritance records. Source nominals must retain both records together.
#[derive(Clone, Copy, Debug)]
pub enum CheckedTypeSelectionDefinitionV1<'a> {
    LanguageBuiltin(CoreBuiltinNominal),
    SourceNominal {
        representation: &'a NominalRepresentationSupportV1,
        inheritance: &'a NominalInheritanceInterfaceV1,
    },
}

impl<'a> CheckedTypeSelectionTargetV1<'a> {
    pub const fn request(self) -> SelectedExternalTypeUseV1 {
        self.request
    }
    /// The defining type, which can differ from a member/slot receiver.
    pub const fn definition(self) -> CheckedTypeSelectionDefinitionV1<'a> {
        self.definition
    }
    pub const fn facts(self) -> CheckedExactTypeFactV1<'a> {
        self.facts
    }
    pub const fn public(self) -> CheckedTypeSectionPublicSupportV1<'a> {
        self.public
    }
    pub const fn declarations(self) -> CheckedProtectedDeclarationSourcesV1<'a> {
        self.declarations
    }
}

/// An authorized persistent external target. Every incoming source/semantic
/// edge has passed replay; the terminal section is retained without copying.
#[derive(Clone, Copy, Debug)]
pub struct CheckedSelectedTypeUseV1<'a> {
    pub(super) target: CheckedTypeSelectionTargetV1<'a>,
    pub(super) terminal: &'a CheckedCrossConeTypeSemanticsSectionV1<'a>,
}
impl<'a> CheckedSelectedTypeUseV1<'a> {
    pub const fn target(self) -> CheckedTypeSelectionTargetV1<'a> {
        self.target
    }
    pub const fn terminal(self) -> &'a CheckedCrossConeTypeSemanticsSectionV1<'a> {
        self.terminal
    }
}

pub(super) use closure::validate;
