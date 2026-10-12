//! Static target contracts shared by codegen and artifact consumers.
//! This crate contains no LLVM initialization, ABI classifier, or tool execution.

mod backend;
pub use backend::*;

mod selection;
pub use selection::*;

mod layout;
pub use layout::*;

mod c_bridge;
pub use c_bridge::*;
