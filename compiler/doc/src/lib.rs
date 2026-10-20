//! Documentation syntax and presentation, independent of compiler stages.

mod markdown;
mod model;
mod normalize;
mod paths;
mod project;
mod syntax;
mod tags;

pub use model::*;
pub use project::{ProjectionError, project_cone};
pub use syntax::prepare_source;

#[cfg(test)]
mod tests;
