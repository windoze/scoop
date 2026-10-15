//! Shared foundation and output relations used by the current Compile reader.

use crate::ValidatedGraphArtifact;
use scoop_hir::{DecodedHirFoundation, HirFoundationValidationError, OdrFreeHirFoundationError};
use scoop_identity::ValidatedIdentityGraph;
use scoop_lir::{
    ConeLirFoundation, ConeLirFoundationError, DecodedLirFoundation, LirFoundationValidationError,
};
use scoop_mir::{DecodedMirFoundation, MirFoundationValidationError, OdrFreeMirFoundationError};
use std::fmt;
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct CanonicalFoundationSet {
    pub(crate) hir: Rc<scoop_hir::CanonicalHirFoundation>,
    pub(crate) mir: Rc<scoop_mir::CanonicalMirFoundation>,
    pub(crate) lir: ConeLirFoundation,
}

mod errors;
mod validation;
pub use errors::StrongProfileFoundationError;
pub(crate) use validation::validate_cross_cone_profile_foundations;

#[cfg(test)]
pub(crate) mod tests;
