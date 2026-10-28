//! Offline HTML rendering from complete, already checked documentation inputs.

use crate::model::*;
use serde::Serialize;
use std::collections::BTreeMap;

mod catalog;
mod markdown;
mod pages;
mod roots;
mod routes;
mod search;
mod views;

pub use catalog::{ConeLinkInputs, SiteCatalog};
pub use roots::{RootReference, render_roots};

#[derive(Clone, Copy, Debug, Default)]
pub struct SiteOptions {
    pub document_private_items: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ConeOverview {
    pub readme: Option<ReadmeInput>,
    pub dependencies: Vec<ConeReference>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConeReference {
    pub key: ConeKey,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReadmeInput {
    pub text: String,
    /// Original Markdown URL to site-relative copied resource. None means a
    /// missing resource for which the caller has already issued a diagnostic.
    pub resources: BTreeMap<String, Option<String>>,
}

#[derive(Debug)]
pub struct RenderedFile {
    pub path: String,
    pub contents: Vec<u8>,
}

#[derive(Debug)]
pub enum RenderError {
    Template(minijinja::Error),
    Encoding(serde_json::Error),
    Address(scoop_wire::HashError),
    DuplicateCone(ConeKey),
    MissingCone(ConeKey),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Template(error) => write!(f, "documentation template: {error}"),
            Self::Encoding(error) => write!(f, "documentation index: {error}"),
            Self::Address(error) => write!(f, "documentation route: {error}"),
            Self::DuplicateCone(key) => write!(f, "duplicate documentation Cone: {}", key.0),
            Self::MissingCone(key) => {
                write!(f, "documentation Cone is not in this site: {}", key.0)
            }
        }
    }
}
impl std::error::Error for RenderError {}
impl From<minijinja::Error> for RenderError {
    fn from(error: minijinja::Error) -> Self {
        Self::Template(error)
    }
}
impl From<scoop_wire::HashError> for RenderError {
    fn from(error: scoop_wire::HashError) -> Self {
        Self::Address(error)
    }
}
type Result<T> = std::result::Result<T, RenderError>;

pub struct SiteRenderer<'catalog, 'input> {
    catalog: &'catalog SiteCatalog<'input>,
    templates: minijinja::Environment<'static>,
    root: Option<String>,
}

impl<'catalog, 'input> SiteRenderer<'catalog, 'input> {
    pub fn new(catalog: &'catalog SiteCatalog<'input>) -> Result<Self> {
        let mut templates = minijinja::Environment::new();
        templates.set_auto_escape_callback(|_| minijinja::AutoEscape::Html);
        templates.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
        for (name, source) in [
            ("frame", include_str!("render/templates/frame.html")),
            ("listing", include_str!("render/templates/listing.html")),
            ("items", include_str!("render/templates/items.html")),
            ("source", include_str!("render/templates/source.html")),
            ("search", include_str!("render/templates/search.html")),
        ] {
            templates.add_template(name, source)?;
        }
        Ok(Self {
            catalog,
            templates,
            root: None,
        })
    }

    /// The caller publishes this catalog below r/<root-key>/ in a larger site.
    pub fn with_root(mut self, name: String) -> Self {
        self.root = Some(name);
        self
    }

    fn template(&self, name: &str, context: impl Serialize) -> Result<String> {
        Ok(self
            .templates
            .get_template(name)?
            .render(minijinja::value::Serde(context))?)
    }
}

#[cfg(test)]
mod tests;
