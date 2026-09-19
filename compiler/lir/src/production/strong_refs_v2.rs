//! Versioned references owned by `strong-production/2`.
//!
//! Resolving identities only authenticates their canonical keys. The complete
//! production reader must additionally join dependency references to the same
//! provider's selected layout/ABI and physical Strong definition proofs.

use scoop_identity::{ConeIdentity, PersistentCallableBodyId, PersistentExactTypeId};
use scoop_wire::{Decoder, Encoder, WireEncode, WireError, WireErrorKind};

use crate::RuntimeFunction;

mod decode;
pub use decode::*;
mod definitions;
pub use definitions::*;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongTypeDescriptorRefV2 {
    Local(PersistentExactTypeId),
    CoreExternal(PersistentExactTypeId),
    DependencyExternal {
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    },
}

impl StrongTypeDescriptorRefV2 {
    pub const fn exact_type(self) -> PersistentExactTypeId {
        match self {
            Self::Local(exact)
            | Self::CoreExternal(exact)
            | Self::DependencyExternal { exact, .. } => exact,
        }
    }
}

impl WireEncode for StrongTypeDescriptorRefV2 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Local(exact) => value(e, 1, exact),
            Self::CoreExternal(exact) => value(e, 2, exact),
            Self::DependencyExternal { provider, exact } => dependency(e, 3, provider, exact),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionalStrongTypeDescriptorRefV2 {
    Absent,
    Local(PersistentExactTypeId),
    CoreExternal(PersistentExactTypeId),
    DependencyExternal {
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    },
}

impl WireEncode for OptionalStrongTypeDescriptorRefV2 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => {
                tag(e, 2, 1)?;
                e.field(1)?;
                e.unsigned(0)
            }
            Self::Local(exact) => value(e, 2, exact),
            Self::CoreExternal(exact) => value(e, 3, exact),
            Self::DependencyExternal { provider, exact } => dependency(e, 4, provider, exact),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongTypeDispatchCallableRefV2 {
    Local(PersistentCallableBodyId),
    CoreExternal(PersistentCallableBodyId),
    Runtime(RuntimeFunction),
    DependencyExternal {
        provider: ConeIdentity,
        body: PersistentCallableBodyId,
    },
}

impl WireEncode for StrongTypeDispatchCallableRefV2 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Local(body) => value(e, 1, body),
            Self::CoreExternal(body) => value(e, 2, body),
            Self::Runtime(function) => runtime(e, *function),
            Self::DependencyExternal { provider, body } => dependency(e, 4, provider, body),
        }
    }
}

fn tag(e: &mut Encoder, fields: u64, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    e.map(fields)?;
    e.field(0)?;
    e.unsigned(tag)
}

fn value(
    e: &mut Encoder,
    kind: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    tag(e, 2, kind)?;
    e.field(1)?;
    value.encode(e)
}

fn dependency(
    e: &mut Encoder,
    kind: u64,
    provider: &impl WireEncode,
    target: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    tag(e, 3, kind)?;
    e.field(1)?;
    provider.encode(e)?;
    e.field(2)?;
    target.encode(e)
}

fn runtime(
    e: &mut Encoder,
    function: RuntimeFunction,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    tag(e, 2, 3)?;
    e.field(1)?;
    e.map(2)?;
    e.field(1)?;
    e.unsigned(function.wire_family_tag())?;
    e.field(2)?;
    e.unsigned(function.wire_function_tag())
}

fn require_fields(d: &Decoder<'_, '_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(error(d, WireErrorKind::InvalidLength { expected, actual }))
    }
}

fn error(d: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, d.path().clone(), Some(d.position()))
}
