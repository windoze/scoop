//! Versioned subprocess protocol between `scoop` and `scoopc`
//! (`docs/milestone23/DESIGN.md` section 5.5).
//!
//! Every message is a single canonical-CBOR frame (little-endian `u32`
//! byte length prefix, then the encoded message). The parent validates
//! [`CompilerIdentity`] before trusting any completion record; a
//! mismatched toolchain is refused instead of worked around.

mod message;

#[cfg(test)]
mod tests;

pub use message::{
    BuildOutcome, BuildRequest, CompilerIdentity, DiagnosticPhase, Hello, Message,
    ProtocolDiagnostic, ProtocolError, Severity, SourceLocation, read_frame, write_frame,
};

/// The only subprocess protocol version this toolchain speaks.
pub const PROTOCOL_VERSION: u32 = 1;

/// Upper bound for one protocol frame; malformed peers cannot force
/// unbounded allocations.
pub const MAX_FRAME_BYTES: u32 = 16 * 1024 * 1024;

/// Structural nesting budget for protocol decoding.
pub const MAX_NESTING: u32 = 16;
