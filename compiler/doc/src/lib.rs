//! Documentation syntax and presentation, independent of compiler stages.

mod markdown;
mod model;
mod normalize;
mod paths;
mod project;
mod render;
mod syntax;
mod tags;

pub use model::*;
pub use project::{ProjectionError, project_cone};
pub use render::{
    ConeLinkInputs, ConeOverview, ConeReference, ReadmeInput, RenderError, RenderedFile,
    SiteCatalog, SiteOptions, SiteRenderer,
};
pub use syntax::prepare_source;

#[cfg(test)]
mod tests;
