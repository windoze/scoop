//! Frontend protocol references and machine selections share the dependency closure.

use scoop_hir::ImportedCoreInputs;
use scoop_identity::ConeIdentity;

use super::ValidatedCrossConeSemanticClosure;

mod errors;
pub use errors::CrossConeProtocolImportError;

impl ValidatedCrossConeSemanticClosure {
    /// Imports only the frontend roles from an already validated provider.
    pub fn import_compiler_protocols(
        &self,
        provider: ConeIdentity,
    ) -> Result<ImportedCoreInputs, CrossConeProtocolImportError> {
        let artifact = self
            .provider(provider)
            .ok_or(CrossConeProtocolImportError::MissingProvider(provider))?;
        let definitions = artifact
            .production()
            .hir_core()
            .compiler_protocols()
            .ok_or(CrossConeProtocolImportError::MissingDefinitions(provider))?;
        artifact
            .hir()
            .import_core_inputs(definitions)
            .map_err(CrossConeProtocolImportError::Import)
    }
}
