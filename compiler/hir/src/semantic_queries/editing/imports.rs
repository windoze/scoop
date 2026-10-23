use crate::{
    EditingImportAliasId, EditingReferenceTarget, ImportedTarget, ReexportRouteV1,
    SemanticSourceLocation,
};
use scoop_identity::PersistentExportBindingId;

/// Sources retained by a successful ordinary import lookup layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditingImportSource {
    Declaration(SemanticSourceLocation),
    Dependency {
        import: Option<SemanticSourceLocation>,
        target: ImportedTarget,
        routes: Vec<ReexportRouteV1>,
    },
}

/// A selected dependency name, before local aliases or re-exports change it.
#[derive(Clone, Debug)]
pub struct EditingExportReference {
    pub target: EditingReferenceTarget,
    pub origin: SemanticSourceLocation,
    pub routes: Vec<ReexportRouteV1>,
    pub aliases: Vec<SemanticSourceLocation>,
}

/// An existing public binding associated with its original source import.
#[derive(Clone, Debug)]
pub struct EditingReexport {
    pub binding: PersistentExportBindingId,
    pub target: EditingReferenceTarget,
    pub origin: SemanticSourceLocation,
    /// Star imports have no source token for each exported name.
    pub name: Option<SemanticSourceLocation>,
    pub alias: Option<EditingImportAliasId>,
    pub routes: Vec<ReexportRouteV1>,
}
