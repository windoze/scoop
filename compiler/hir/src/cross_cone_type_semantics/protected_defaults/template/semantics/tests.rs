use super::*;
use crate::*;
use scoop_identity::*;
use scoop_identity::{DefinitionOrigin, GcEffect, NonEmptyVec};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

mod authority;
mod contracts;
mod origins;
mod sources;
mod support;
use authority::Authority;
use support::{Case, binder, local, meter};
