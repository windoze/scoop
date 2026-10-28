//! Documentation syntax and presentation, independent of compiler stages.

mod hover;
mod markdown;
pub use hover::hover_markdown;
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
    RootReference, SiteCatalog, SiteOptions, SiteRenderer, render_roots,
};
pub use syntax::{prepare_comments, prepare_source};

#[cfg(test)]
mod tests;
