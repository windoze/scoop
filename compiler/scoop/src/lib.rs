//! `scoop` umbrella build tool: multi-Cone graph resolution, caching and
//! scheduling over the single-Cone `scoopc` artifact boundary
//! (`docs/milestone23/DESIGN.md` sections 1.4 and 5.5).
//!
//! The library depends only on manifest/protocol/slib-level types; it
//! never links compiler stage implementations. Subprocess orchestration
//! lives behind traits so the graph layer stays testable in memory.

pub mod graph;
#[cfg(test)]
mod graph_tests;

pub use graph::{
    BuildInputs, GraphError, NodeOrigin, ResolvedBuildGraph, ResolvedNode, resolve_graph,
};
