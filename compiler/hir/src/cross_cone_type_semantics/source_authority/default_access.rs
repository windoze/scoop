//! Default-provider access snapshots, without access or default authority.

mod domain;
mod errors;
mod replay;
mod witness;
pub use domain::*;
pub use errors::*;
pub use replay::DefaultSourceDomainReplayError;
pub(super) use replay::lookup_domain;
pub use witness::*;
