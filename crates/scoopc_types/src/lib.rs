//! Shared type-system foundations for the compiler pipeline.
//!
//! This base crate will own `TypeId`, `TypeStore`, `TypeKind`, `EffectRow`, the
//! builtin type universe, and backend-neutral type layout data. Stage and fact
//! crates may share these definitions through this crate instead of depending on
//! each other. It must not depend on `scoopc`, stage crates, fact crates,
//! backend crates, or repository tools.
//!
//! P1-T01 provides only the shell and dependency boundary; P1-T03 migrates the
//! current authoritative type definitions here.

#![forbid(unsafe_code)]
