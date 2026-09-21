//! Source value identities retained independently of default expansion storage.

use crate::{BindingId, LexicalDefinitionRoot, LocalValueDefinitionSite};
use scoop_identity::{LocalValueSelector, StructuralDefinitionPath};

pub type DefaultLocalValueScopeId = la_arena::Idx<DefaultLocalValueScope>;

#[derive(Debug, Clone)]
pub struct DefaultLocalValueScope {
    pub definition_root: LexicalDefinitionRoot,
    pub definition_path: StructuralDefinitionPath,
    pub values: Vec<DefaultLocalValueDefinition>,
}

#[derive(Debug, Clone)]
pub struct DefaultLocalValueDefinition {
    pub binding: BindingId,
    pub selector: LocalValueSelector,
    pub definition: LocalValueDefinitionSite,
}
