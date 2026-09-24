use super::*;
use scoop_identity::{InitializationCallableRole, PersistentObjectValueId};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

mod budget;
mod callables;
mod closure;
mod core;
mod identity_support;
mod producer;
mod resolved_dependencies;
mod source;
mod support;
mod units;
mod wire;

use source::Source;
use support::*;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
