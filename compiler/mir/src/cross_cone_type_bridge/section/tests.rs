use super::*;
use scoop_identity::InitializationCallableRole;
use scoop_wire::{decode_canonical, encode};

mod callables;
mod closure;
mod core;
mod identity_support;
mod resolved_dependencies;
mod support;
mod units;
mod wire;

use support::*;
