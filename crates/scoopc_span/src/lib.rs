//! Stage-independent span and diagnostic coordinate foundations.
//!
//! This base crate will own source span primitives that every compiler stage and
//! fact crate may share. It must not depend on `scoopc`, stage crates, fact
//! crates, backend crates, or repository tools.
//!
//! P1-T01 intentionally leaves the business types in `scoopc`; later P1 tasks
//! migrate the authoritative definitions here without introducing duplicate
//! span types.

#![forbid(unsafe_code)]
