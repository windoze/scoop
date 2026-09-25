//! Complete semantic section with independent source and terminal closure.

use super::*;
use scoop_identity::{ConeIdentity, PersistentInitializationUnitId, StrongCallableDefinitionOwner};

mod build;
mod closure;
mod context;
mod dependencies;
mod dependency;
mod error;
mod model;
mod replay;
mod resolved_dependencies;
mod selection;
mod source;
#[cfg(test)]
mod tests;
mod units;
mod view;
mod wire;

pub use dependency::MirTypeBridgeDependencyV1;
pub use error::*;
pub use model::*;
pub use resolved_dependencies::DependencyResolvedCrossConeMirTypeBridgeSectionV1;
pub use selection::{SelectedDependencyMirTypeRefV1, SelectedDependencyMirTypeSetV1};
pub use source::*;
pub use units::{
    MirInitializationUnitProofKindV1, MirTypeBridgeInitializationUnitV1,
    replay_source_initialization_units,
};
pub use view::MirTypeBridgeDependencyViewV1;
pub use wire::{
    CallablesResolvedCrossConeMirTypeBridgeSectionV1, DecodedCrossConeMirTypeBridgeSectionV1,
    TypeResolvedCrossConeMirTypeBridgeSectionV1,
};

fn reserve<T>(count: usize) -> Result<Vec<T>, WireError> {
    let mut values = Vec::new();
    scoop_wire::allocation::try_reserve(&mut values, count, &WirePath::root())?;
    Ok(values)
}
