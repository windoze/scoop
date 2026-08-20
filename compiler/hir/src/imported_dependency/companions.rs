//! A dependency companion reuses its original constructor template and unit.

use std::sync::Arc;

use crate::*;
use scoop_identity::{CborIdentityRecord, GeneratedCallableKey, PersistentGeneratedCallableId};

#[derive(Debug, Clone)]
pub struct ImportedCompanionTemplate {
    pub declaration: Arc<ImportedNominalDeclaration>,
    pub host: Arc<ImportedNominalDeclaration>,
    pub constructor: ImportedConstructorTemplateId,
    /// Direct generic singleton reads in the original initializer body.
    pub dependencies: Vec<TypeId>,
    pub signature: CallableSignature,
    pub display_name: String,
    pub origin: DefinitionOrigin,
    pub initializer: CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>,
    pub ensure: CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>,
}

impl ImportedCompanionTemplate {
    pub fn callable(
        &self,
        role: scoop_identity::InitializationCallableRole,
    ) -> &CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey> {
        match role {
            scoop_identity::InitializationCallableRole::Initializer => &self.initializer,
            scoop_identity::InitializationCallableRole::Ensure => &self.ensure,
        }
    }
}
