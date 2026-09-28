//! Portable implementation templates. Expression and statement nodes are the
//! same typed nodes used by source defaults; each kind retains its own root.

mod callable;
mod delegate;
mod fragment;
mod initialization;
mod predicates;
mod references;
mod table;

pub use callable::*;
pub use delegate::*;
pub use fragment::*;
pub use initialization::*;
pub use predicates::*;
pub use table::*;
