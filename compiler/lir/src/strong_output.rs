//! Sealed LIR output for the single-Cone strong production path.

use crate::{Module, OdrFreeLirFoundation, OdrFreeLirFoundationProjectionError};

/// One LIR graph paired with the exact ODR-free foundation projected from it.
///
/// The fields are private so production orchestration cannot replace either
/// half after the strong lowering gate has succeeded.
pub struct SingleConeStrongLirOutput {
    module: Module,
    foundation: OdrFreeLirFoundation,
}

impl SingleConeStrongLirOutput {
    pub fn try_new(module: Module) -> Result<Self, OdrFreeLirFoundationProjectionError> {
        let foundation = OdrFreeLirFoundation::from_module(&module)?;
        Ok(Self { module, foundation })
    }

    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundation
    }

    pub fn into_module(self) -> Module {
        self.module
    }
}
