use std::fmt;
use std::rc::Rc;

use scoop_hir::{
    CoreHirInterfaceBranchV1, CoreHirInterfaceV1, CoreInterfaceImportError, ImportedCoreInputs,
};
use scoop_identity::{
    ConeIdentity, CoreBuiltinNominal, Effect, ExactCallableSignature, ExactTypeKey,
    PersistentExactTypeId,
};
use scoop_lir::{ImportedLirCallableProjectionError, ImportedLirTypeDescriptorProjectionError};
use scoop_mir::ImportedMirCallableProjectionError;
use scoop_slib::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CrossConeSemanticsStrongProfile, SharedCrossConeArtifact,
    ValidatedCompileArtifact, ValidatedCrossConeArtifactClosure,
};

mod projection;
pub use projection::*;

pub struct ValidatedTrustedCoreArtifact<'input> {
    artifact: SharedCrossConeArtifact<'input>,
    interface: CoreHirInterfaceV1,
}

impl<'input> ValidatedTrustedCoreArtifact<'input> {
    pub(crate) fn from_closure(
        closure: &Rc<ValidatedCrossConeArtifactClosure<'input>>,
    ) -> Result<Self, TrustedCoreArtifactValidationError> {
        let artifact = closure
            .share_artifact(ConeIdentity::CORE)
            .ok_or(TrustedCoreArtifactValidationError::MissingCore)?;
        let compile = artifact.compile();
        let interface = match compile.production().hir_core().core_interface() {
            CoreHirInterfaceBranchV1::Core(interface) => interface.as_ref().clone(),
            CoreHirInterfaceBranchV1::NotCore => {
                return Err(TrustedCoreArtifactValidationError::MissingCoreInterface);
            }
        };
        Ok(Self {
            artifact,
            interface,
        })
    }

    pub(crate) fn compile(
        &self,
    ) -> &ValidatedCompileArtifact<'input, CrossConeSemanticsStrongProfile> {
        self.artifact.compile()
    }

    /// Projects compiler protocols and native-boundary definitions from the shared
    /// dependency artifact and its decoded interface.
    pub fn import_core_inputs(&self) -> Result<ImportedCoreInputs, CoreInterfaceImportError> {
        self.compile().hir().import_core_inputs(&self.interface)
    }

    pub fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        self.artifact.link().defined_symbols()
    }
}

#[derive(Debug)]
pub enum TrustedCoreArtifactValidationError {
    MissingCore,
    MissingCoreInterface,
}

impl fmt::Display for TrustedCoreArtifactValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCore => formatter.write_str("dependency closure has no core artifact"),
            Self::MissingCoreInterface => {
                formatter.write_str("trusted core Compile proof has no Core HIR interface")
            }
        }
    }
}

impl std::error::Error for TrustedCoreArtifactValidationError {}
