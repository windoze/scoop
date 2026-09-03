//! Unified source-call resolution model introduced by M16.
//!
//! The first implementation slice keeps the existing applicability engine,
//! but all declaration candidates are normalized through [`CallableView`]
//! and receive their own semantic argument map before that engine runs.

pub(crate) mod arguments;
pub(crate) mod candidates;
