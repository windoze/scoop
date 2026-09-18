//! Winner-only materialization of validated dependency default templates.

mod capability;
mod materialize;
mod origins;
mod plan;
mod preflight;

pub(super) use plan::ImportedDefaultPlan;
