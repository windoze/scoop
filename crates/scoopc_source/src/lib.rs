//! Source identity and source-map foundations for cone compilation units.
//!
//! This base crate will own `SourceId`, source files, source maps, source trust,
//! and stable source membership data that can be consumed by stages and facts.
//! It may depend on `scoopc_span` once those definitions move, but it must not
//! depend on `scoopc`, parser/resolve/typecheck/HIR/MIR/LIR/codegen crates, fact
//! crates, or repository tools.
//!
//! P1-T01 provides only the shell and documentation; P1-T02 moves the current
//! authoritative source definitions into this crate.

#![forbid(unsafe_code)]
