//! Shared physical type encoding for callable and shape content hashes.

use crate::*;
use scoop_wire::{Encoder, WireEncode, cbor::EncodeError};

pub(crate) fn encode_type(
    module: &Module,
    ty: &LirType,
    e: &mut Encoder,
) -> Result<(), EncodeError> {
    let (tag, payload) = match ty {
        LirType::Void => (1, false),
        LirType::I1 => (2, false),
        LirType::I8 => (3, false),
        LirType::I16 => (4, false),
        LirType::I32 => (5, false),
        LirType::I64 => (6, false),
        LirType::F32 => (13, false),
        LirType::F64 => (14, false),
        LirType::MachineScalar(_) => (7, true),
        LirType::Ptr(_) => (8, true),
        LirType::ExceptionRecord => (9, false),
        LirType::Aggregate(_) => (10, true),
        LirType::Struct(_) => (11, true),
        LirType::Enum(_) => (12, true),
    };
    e.map(if payload { 2 } else { 1 })?;
    e.field(0)?;
    e.unsigned(tag)?;
    if payload {
        e.field(1)?;
    }
    match ty {
        LirType::MachineScalar(kind) => e.unsigned(machine_kind(*kind)),
        LirType::Ptr(kind) => e.unsigned(pointer_kind(*kind)),
        LirType::Aggregate(elements) => {
            e.array(elements.len() as u64)?;
            for element in elements {
                encode_type(module, element, e)?;
            }
            Ok(())
        }
        LirType::Struct(id) => module.structs[*id].exact_type.encode(e),
        LirType::Enum(id) => module.enums[*id].exact_type.encode(e),
        LirType::Void
        | LirType::I1
        | LirType::I8
        | LirType::I16
        | LirType::I32
        | LirType::F32
        | LirType::F64
        | LirType::I64
        | LirType::ExceptionRecord => Ok(()),
    }
}

pub(crate) const fn pointer_kind(kind: PointerKind) -> u64 {
    match kind {
        PointerKind::Managed => 1,
        PointerKind::Raw => 2,
        PointerKind::Code => 3,
        PointerKind::Metadata => 4,
    }
}

pub(crate) const fn machine_kind(kind: MachineScalarKind) -> u64 {
    match kind {
        MachineScalarKind::ByteSize => 1,
        MachineScalarKind::EnumTag => 2,
        MachineScalarKind::InitializationOutcome => 3,
        MachineScalarKind::CoroutineFrameState => 4,
        MachineScalarKind::CoroutineAdapterState => 5,
        MachineScalarKind::ForeignCallbackStatus => 6,
        MachineScalarKind::PointerElementOffset => 7,
    }
}

pub(crate) fn encode_integer(kind: IntegerKind, e: &mut Encoder) -> Result<(), EncodeError> {
    e.map(2)?;
    e.field(0)?;
    e.unsigned(match kind.signedness() {
        IntegerSignedness::Signed => 1,
        IntegerSignedness::Unsigned => 2,
    })?;
    e.field(1)?;
    e.unsigned(u64::from(kind.width().bits()))
}
