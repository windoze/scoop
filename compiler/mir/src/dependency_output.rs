//! Complete current-Cone MIR and the identity and dependency data used downstream.

use std::fmt;
use std::rc::Rc;

use crate::{
    CanonicalMirFoundation, MirFoundationBuildError, Module, OdrFreeMirFoundation,
    OdrFreeMirFoundationError, SelectedExternalMirSet, StrongExternalCallableRoot,
};

/// Complete MIR graph paired with the one selection resolving every external use.
pub struct DependencyMirOutput {
    module: Module,
    foundation: Rc<CanonicalMirFoundation>,
    selected_callables: SelectedExternalMirSet,
    external_callable_roots: Vec<StrongExternalCallableRoot>,
}

impl DependencyMirOutput {
    pub fn try_new(
        module: Module,
        selected_callables: SelectedExternalMirSet,
    ) -> Result<Self, DependencyMirOutputError> {
        let foundation = CanonicalMirFoundation::from_module(&module)
            .map_err(DependencyMirOutputError::Foundation)?;
        let external_callable_roots =
            crate::strong_input::validate_external_callables(&module, &selected_callables)
                .map_err(DependencyMirOutputError::ExternalCallables)?;

        Ok(Self {
            module,
            foundation: Rc::new(foundation),
            selected_callables,
            external_callable_roots,
        })
    }

    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn selected_callables(&self) -> &SelectedExternalMirSet {
        &self.selected_callables
    }

    pub fn strong_foundation(&self) -> Result<OdrFreeMirFoundation, OdrFreeMirFoundationError> {
        OdrFreeMirFoundation::try_from_shared(Rc::clone(&self.foundation))
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Module,
        Rc<CanonicalMirFoundation>,
        SelectedExternalMirSet,
        Vec<StrongExternalCallableRoot>,
    ) {
        (
            self.module,
            self.foundation,
            self.selected_callables,
            self.external_callable_roots,
        )
    }
}

#[derive(Debug)]
pub enum DependencyMirOutputError {
    Foundation(MirFoundationBuildError),
    ExternalCallables(crate::SingleConeStrongMirInputError),
}

impl fmt::Display for DependencyMirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Foundation(source) => source.fmt(formatter),
            Self::ExternalCallables(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for DependencyMirOutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Foundation(source) => Some(source),
            Self::ExternalCallables(source) => Some(source),
        }
    }
}
