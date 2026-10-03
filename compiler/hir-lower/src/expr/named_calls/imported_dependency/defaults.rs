//! Load default definitions and expand the shared HIR body for selected calls.

mod materialize;
mod plan;
mod prepare;

pub(super) use plan::ImportedDefaultPlan;
