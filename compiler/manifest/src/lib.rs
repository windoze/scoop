//! `Cone.toml` v1 parsing with strict field checking and spanned
//! diagnostics (`docs/milestone23/DESIGN.md` section 1.2).
//!
//! One parse produces both projections of the manifest: the **semantic
//! projection** (coordinate, kind, exact dependency coordinates) that
//! `scoopc` consumes, and the **locator projection** (path/artifact
//! locators) that only `scoop` interprets. Locators never participate in
//! Cone identity, artifact metadata or diagnostics source identity.

mod model;
mod parse;

#[cfg(test)]
mod tests;

pub use model::{ConeKind, ConeManifest, DeclaredDependency, DependencyLocator};
pub use parse::{ManifestDiagnostic, parse_manifest};

/// The only manifest schema version accepted by this implementation.
pub const MANIFEST_SCHEMA_VERSION: i64 = 1;
