//! Complete callable binding constituents; production joins remain explicit.

use super::*;
use scoop_identity::{
    DecodedStrongCallableDefinitionOwner, DispatchDeclarationOwner, ExactCallableSignature,
    GeneratedCallableKey, PersistentConstructorId, PersistentDispatchSlotId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentInitializationUnitId, PersistentPropertyAccessorId,
    StrongCallableDefinitionOwner,
};

mod boxing;
mod model;
mod origin;
mod roles;
mod signature;
#[cfg(test)]
mod tests;
mod validation;
mod wire;

pub use boxing::MirBoxingCallableProductionError;
pub use model::*;
pub use origin::*;
pub use roles::*;
pub use signature::*;
pub use validation::MirCallableBridgeError;
pub use wire::{DecodedCanonicalMirCallableBindingsV1, DecodedParamFreeMirCallableBindingV1};
