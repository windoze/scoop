//! HIR-lowering implementation split into focused submodules.

#![allow(dead_code)]

use super::*;

mod accessors;
mod compilation_unit;
mod entry;
mod helpers;
mod impl_lowering;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use {accessors::*, compilation_unit::*, entry::*, helpers::*, impl_lowering::*};
