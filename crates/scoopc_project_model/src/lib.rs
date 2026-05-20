//! Stage-independent project, source-cone, and compilation-unit model.
//!
//! This base crate will own project membership, source-cone graph data,
//! compilation-unit membership, source trust, and dependency topology checks. It
//! may depend on source, type, and ID base crates as those definitions migrate,
//! but it must not depend on `scoopc`, filesystem/session loaders, stage crates,
//! fact crates, backend crates, or repository tools.
//!
//! P1-T01 provides only the shell and dependency boundary; later P1 tasks move
//! the authoritative project/cone model here.

#![forbid(unsafe_code)]
