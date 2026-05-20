//! Stable identity primitives shared across compiler stages and facts.
//!
//! This base crate will own cross-stage identifiers such as site IDs, stable
//! hash/key primitives, and body-version keys. It must not depend on `scoopc`,
//! stage crates, fact crates, backend crates, or repository tools.
//!
//! P1-T01 provides only the shell and migration target; P1-T03 introduces the
//! first authoritative identity definitions here.

#![forbid(unsafe_code)]
