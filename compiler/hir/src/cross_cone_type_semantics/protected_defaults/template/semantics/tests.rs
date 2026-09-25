use super::*;
use crate::*;
use scoop_identity::*;
use scoop_identity::{GcEffect, NonEmptyVec};

mod authority;
mod contracts;
mod origins;
mod provider_parameters;
mod provider_receiver;
mod sources;
mod support;
use authority::Authority;
use support::{Case, binder, local};
