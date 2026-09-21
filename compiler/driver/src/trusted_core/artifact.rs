use std::fmt;
use std::rc::Rc;

use scoop_hir::{
    CoreCallableDefinitionV1, CoreHirCallableCapabilityV1, CoreHirInterfaceBranchV1,
    CoreHirInterfaceV1, CoreInterfaceImportError, ImportedCoreInputs, ImportedCorePreludeTarget,
    SelectedImportedCoreSet, SelectedImportedCoreTarget,
};
use scoop_identity::{
    ConeIdentity, CoreBuiltinNominal, CoreImportedCallableKind, Effect, ExactCallableSignature,
    ExactTypeKey, PersistentExactTypeId, PersistentExportBindingId,
};
use scoop_lir::{
    ImportedLirCallableProjectionError, ImportedLirSelectionError,
    ImportedLirTypeDescriptorProjectionError, SelectedImportedLirCallable, SelectedImportedLirSet,
};
use scoop_mir::{
    CoreMirBridgeBranchV1, ImportedMirCallableProjectionError, ImportedMirSelectionError,
    SelectedImportedMirCallable, SelectedImportedMirSet,
};
use scoop_slib::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CrossConeSemanticsStrongProfile, SharedCrossConeArtifact,
    ValidatedCompileArtifact, ValidatedCrossConeArtifactClosure,
};

mod projection;
pub use projection::*;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ValidatedCoreInterface {
    interface: CoreHirInterfaceV1,
    strong_callable_bindings: Vec<PersistentExportBindingId>,
}

pub struct ValidatedTrustedCoreArtifact<'input> {
    artifact: SharedCrossConeArtifact<'input>,
    core_interface: ValidatedCoreInterface,
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
        let strong_callable_bindings = match compile.production().mir_core().core_bridge() {
            CoreMirBridgeBranchV1::Core(bridge) => bridge
                .callable_targets()
                .iter()
                .map(|target| target.binding())
                .collect(),
            CoreMirBridgeBranchV1::NotCore => {
                return Err(TrustedCoreArtifactValidationError::MissingCoreMirBridge);
            }
        };
        Ok(Self {
            artifact,
            core_interface: ValidatedCoreInterface {
                interface,
                strong_callable_bindings,
            },
        })
    }

    pub(crate) fn compile(
        &self,
    ) -> &ValidatedCompileArtifact<'input, CrossConeSemanticsStrongProfile> {
        self.artifact.compile()
    }

    /// Atomically projects the only HIR lookup and compiler-protocol
    /// capabilities authorized for an M23-3 consumer from this artifact's
    /// own Compile proof and core interface.
    pub fn import_core_inputs(&self) -> Result<ImportedCoreInputs<'_>, CoreInterfaceImportError> {
        self.compile().hir().import_core_inputs(
            &self.core_interface.interface,
            &self.core_interface.strong_callable_bindings,
        )
    }

    pub fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        self.artifact.link().defined_symbols()
    }
}

#[derive(Debug)]
pub enum TrustedCoreArtifactValidationError {
    MissingCore,
    MissingCoreInterface,
    MissingCoreMirBridge,
}

impl fmt::Display for TrustedCoreArtifactValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCore => formatter.write_str("dependency closure has no core artifact"),
            Self::MissingCoreInterface => {
                formatter.write_str("trusted core Compile proof has no Core HIR interface")
            }
            Self::MissingCoreMirBridge => {
                formatter.write_str("trusted core Compile proof has no Core MIR bridge")
            }
        }
    }
}

impl std::error::Error for TrustedCoreArtifactValidationError {}
