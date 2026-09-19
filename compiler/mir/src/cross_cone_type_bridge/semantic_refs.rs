//! Complete direct semantic edges, without source or selection authority.

use super::*;
use scoop_identity::{
    DispatchDeclarationOwner, DispatchSlotKey, GeneratedCallableKey, InitializationUnitKey,
    PersistentInitializationUnitId, PersistentObjectValueId, StrongCallableDefinitionOwner,
};

mod collector;
mod records;
mod target;
mod wire;

pub use collector::{MirTypeBridgeReferenceError, MirTypeBridgeSemanticReferencesV1};
pub use target::{DecodedMirTypeBridgeTargetV1, MirTypeBridgeTargetV1};

#[cfg(test)]
mod tests;
