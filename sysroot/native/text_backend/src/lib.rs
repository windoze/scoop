//! Internal, synchronous text operations for Scoop's ordinary C FFI.
//!
//! Managed input is borrowed only for the current call. Owned results must be
//! returned to `scoop_text_free`, never to the caller's C allocator. Upstream
//! allocation failure and internal panic abort; format errors return statuses.

mod bytes;
mod domain;
mod normalize;
mod punycode;

mod charset;
