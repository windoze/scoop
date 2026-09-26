//! Source-side access snapshots, preserving symbolic generic class regions.

use crate::*;
use scoop_wire::WirePath;

mod call_domains;
mod domains;
pub(super) use call_domains::SourceCallDomain;
mod errors;
mod shared;
use DefaultSourceAccessProductionError as Error;
pub use errors::DefaultSourceAccessProductionError;
