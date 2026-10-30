//! Static target contracts shared by codegen and artifact consumers.
//! This crate contains no LLVM initialization, ABI classifier, or tool execution.

mod backend;
pub use backend::*;

mod catalog;
pub use catalog::*;

mod layout;

mod c_bridge;
pub use c_bridge::*;
