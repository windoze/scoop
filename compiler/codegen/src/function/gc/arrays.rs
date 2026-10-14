//! Managed array construction and copying, including the complete write barrier.

use super::*;

mod allocation;
mod assembly;
mod clone;
mod dynamic;
