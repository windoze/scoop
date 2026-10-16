//! Dependency graph, imported metadata and the current artifact closure.
mod errors;
pub use errors::*;
#[path = "cross_cone_closure/surface_validation/dependency_views.rs"]
mod dependency_views;
pub(crate) mod graph_validation;
pub(crate) use dependency_views::transitive_dependency_positions;
mod type_alias_expansion;
pub use type_alias_expansion::*;
mod commit;
pub use commit::*;
mod artifact;
pub use artifact::*;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CrossConeProviderRole {
    Direct,
    Support,
}
