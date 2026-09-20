//! Ordered, independent source occurrences. Decoding grants no access authority.
mod errors;
mod record;
mod resolve;
mod sequences;
pub use errors::*;
pub use record::*;
pub use resolve::DefaultSourceReferenceResolver;
pub use sequences::*;
