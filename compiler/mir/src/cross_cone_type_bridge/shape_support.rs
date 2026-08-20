//! Finite, representation-neutral support families for source nominals.
//!
//! These records do not establish source visibility or dependency selection.
//! The complete section must independently prove its required source roots.

use super::*;
use scoop_identity::{ConeIdentity, SourceDeclarationKey};

mod model;
mod production;
mod table;
mod validation;
mod wire;

pub use model::*;
pub use table::*;
pub use validation::MirShapeSupportError;
pub use wire::DecodedParamFreeMirShapeSupportV1;

#[cfg(test)]
mod tests;
