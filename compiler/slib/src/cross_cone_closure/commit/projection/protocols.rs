//! Frontend protocol references and machine selections share the dependency closure.

use scoop_hir::{
    ImportedCoreInputs, ImportedCoreProtocolCallable, ImportedCoreProtocolCallableDefinition,
};
use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId};
use scoop_mir::SelectedExternalMirSet;

use super::ValidatedCrossConeSemanticClosure;

mod errors;
pub use errors::{CrossConeInitializationSelectionError, CrossConeProtocolImportError};

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
            .compiler_protocol_definitions()
            .ok_or(CrossConeProtocolImportError::MissingDefinitions(provider))?;
        artifact
            .hir()
            .import_core_inputs(definitions)
            .map_err(CrossConeProtocolImportError::Import)
    }

    /// Selects the resolved service using its provider's complete MIR signature.
    pub fn select_initialization_cycle(
        &self,
        selected: SelectedExternalMirSet,
        callable: &ImportedCoreProtocolCallable,
    ) -> Result<SelectedExternalMirSet, CrossConeInitializationSelectionError> {
        use CrossConeInitializationSelectionError as Error;

        if selected.consumer() != self.current {
            return Err(Error::ConsumerMismatch {
                closure: self.current,
                selected: selected.consumer(),
            });
        }
        let ImportedCoreProtocolCallableDefinition::Function(imported) = callable.definition()
        else {
            return Err(Error::InvalidDefinition(callable.definition()));
        };
        let provider = callable.provider();
        let definition = imported.persistent();
        let artifact = self
            .provider(provider)
            .ok_or(Error::MissingProvider(provider))?;
        let export = artifact
            .production()
            .mir_cross_cone()
            .export(DependencyCallableDeclarationId::Function(definition))
            .ok_or(Error::MissingMirBridge {
                provider,
                definition,
            })?;
        let record = scoop_mir::SelectedDependencyMirCallableV1::try_new(
            provider,
            export.declaration(),
            export.implementation(),
            export.signature().clone(),
        )
        .map_err(Error::Record)?;
        selected
            .with_initialization_cycle(record)
            .map_err(Error::Selection)
    }
}
