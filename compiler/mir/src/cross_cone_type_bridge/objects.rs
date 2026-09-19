//! Identity-checked singleton read plans and explicit initialization uses.

use super::*;
use scoop_identity::{
    ConeIdentity, GeneratedCallableKey, InitializationCallableRole, InitializationUnitKey,
    PersistentInitializationUnitId, PersistentObjectValueId, PersistentPropertyAccessorId,
    SourceDeclarationKey, StrongCallableDefinitionOwner,
};

mod initialization;
mod model;
mod tables;
mod validation;
mod wire;

pub use initialization::*;
pub use model::*;
pub use tables::*;
pub use validation::MirObjectBridgeError;
pub use wire::{DecodedParamFreeMirObjectValueV1, DecodedSelectedExternalInitializationUseV1};

#[cfg(test)]
mod tests;
