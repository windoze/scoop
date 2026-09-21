//! Current-Cone MIR paired with its exact protocol and dependency selections.

use std::fmt;

mod protocols;
pub use protocols::{CurrentMirProtocolDeclarations, MirProtocolSelection};

use crate::{MirValidationError, Module, SelectedDependencyMirSet};

/// Closed MIR product whose imported protocol references are branded by
/// and resolved through one exact selected set.
///
/// Owning the sidecar prevents later stages from pairing an already-lowered
/// MIR graph with an equal-looking selection projected from another artifact.
pub struct DependencyMirOutput<P: MirProtocolSelection> {
    module: Module,
    protocols: P,
    imported_dependencies: SelectedDependencyMirSet,
}

impl<P: MirProtocolSelection> DependencyMirOutput<P> {
    pub fn try_new(
        module: Module,
        protocols: P,
        imported_dependencies: SelectedDependencyMirSet,
    ) -> Result<Self, DependencyMirOutputError> {
        module
            .validate()
            .map_err(DependencyMirOutputError::InvalidModule)?;
        crate::strong_input::validate_external_callables(
            &module,
            protocols.as_strong_input(),
            crate::StrongImportedDependencyInput::Selected(&imported_dependencies),
        )
        .map_err(DependencyMirOutputError::ExternalCallables)?;

        Ok(Self {
            module,
            protocols,
            imported_dependencies,
        })
    }

    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn protocols(&self) -> &P {
        &self.protocols
    }

    pub const fn imported_dependencies(&self) -> &SelectedDependencyMirSet {
        &self.imported_dependencies
    }

    pub fn into_parts(self) -> (Module, P, SelectedDependencyMirSet) {
        (self.module, self.protocols, self.imported_dependencies)
    }
}

#[derive(Debug)]
pub enum DependencyMirOutputError {
    InvalidModule(MirValidationError),
    ExternalCallables(crate::SingleConeStrongMirInputError),
}

impl fmt::Display for DependencyMirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid current-Cone MIR output: {self:?}")
    }
}

impl std::error::Error for DependencyMirOutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidModule(source) => Some(source),
            Self::ExternalCallables(source) => Some(source),
        }
    }
}
