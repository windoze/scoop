//! Current-Cone MIR paired with its complete external callable selection.

use std::fmt;

use crate::{MirValidationError, Module, SelectedExternalMirSet};

/// Complete MIR graph paired with the one selection resolving every external use.
pub struct DependencyMirOutput {
    module: Module,
    selected_callables: SelectedExternalMirSet,
}

impl DependencyMirOutput {
    pub fn try_new(
        module: Module,
        selected_callables: SelectedExternalMirSet,
    ) -> Result<Self, DependencyMirOutputError> {
        module
            .validate()
            .map_err(DependencyMirOutputError::InvalidModule)?;
        crate::strong_input::validate_external_callables(
            &module,
            crate::StrongExternalCallableInput::Selected(&selected_callables),
        )
        .map_err(DependencyMirOutputError::ExternalCallables)?;

        Ok(Self {
            module,
            selected_callables,
        })
    }

    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn selected_callables(&self) -> &SelectedExternalMirSet {
        &self.selected_callables
    }

    pub fn into_parts(self) -> (Module, SelectedExternalMirSet) {
        (self.module, self.selected_callables)
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
