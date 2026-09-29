//! Unified source-call resolution model introduced by M16.
//!
//! The first implementation slice keeps the existing applicability engine,
//! but all declaration candidates are normalized through [`CallableView`]
//! and receive their own semantic argument map before that engine runs.

pub(crate) mod applicability;
pub(crate) mod arguments;
pub(crate) mod candidates;
pub(crate) mod diagnostics;
pub(crate) mod named;
// The resolver is landing dependency-first. These modules are exercised by
// their solver tests now and become production-reachable as applicability and
// specificity migrate in the following M16 slices.
#[allow(dead_code)]
pub(crate) mod constraints;
pub(crate) mod contextual;
#[allow(dead_code)]
mod relations;
#[allow(dead_code)]
pub(crate) mod solver;
pub(crate) mod specificity;

#[cfg(test)]
mod tests;
