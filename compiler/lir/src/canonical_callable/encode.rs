//! Canonical shared ABI; private instructions, locals and sites are not inputs.

use crate::*;
use scoop_wire::{Encoder, WireEncode, cbor::EncodeError};

type Result = std::result::Result<(), EncodeError>;

pub(super) struct CallableAbiProjection<'a> {
    pub(super) module: &'a Module,
    pub(super) function: &'a Function,
    pub(super) group: scoop_identity::OdrGroupId,
    pub(super) member: scoop_identity::OdrMemberId,
    pub(super) role: scoop_identity::OdrMemberRole,
}

impl WireEncode for CallableAbiProjection<'_> {
    fn encode(&self, e: &mut Encoder) -> Result {
        e.map(4)?;
        e.field(1)?;
        self.group.encode(e)?;
        e.field(2)?;
        self.member.encode(e)?;
        e.field(3)?;
        self.role.encode(e)?;
        e.field(4)?;
        e.map(2)?;
        e.field(1)?;
        e.unsigned(match self.function.gc_effect {
            GcEffect::Managed => 1,
            GcEffect::NoGc => 2,
        })?;
        e.field(2)?;
        let signature = &self.function.signature;
        tagged(e, 1, 3)?;
        e.field(1)?;
        signature.calling_convention().encode(e)?;
        e.field(2)?;
        e.array(signature.arguments().len() as u64)?;
        for argument in signature.arguments() {
            match argument {
                AbiArgument::ElidedZst(value) => {
                    tagged(e, 1, 1)?;
                    e.field(1)?;
                    self.zst(value, e)?;
                }
                AbiArgument::Direct(value) => {
                    self.direct(value, 2, 4, e)?;
                }
                AbiArgument::Indirect(value) => {
                    tagged(e, 3, 1)?;
                    e.field(1)?;
                    self.value(value, e)?;
                }
            }
        }
        e.field(3)?;
        match signature.result() {
            AbiReturn::UnitVoid => tagged(e, 1, 0),
            AbiReturn::ElidedZst(value) => {
                tagged(e, 2, 1)?;
                e.field(1)?;
                self.zst(value, e)
            }
            AbiReturn::Direct(value) => self.direct(value, 3, 5, e),
            AbiReturn::Indirect(value) => {
                tagged(e, 4, 1)?;
                e.field(1)?;
                self.value(value, e)
            }
        }
    }
}

impl CallableAbiProjection<'_> {
    fn direct(
        &self,
        value: &AbiDirectValue,
        scalar_tag: u64,
        parts_tag: u64,
        e: &mut Encoder,
    ) -> Result {
        match value {
            AbiDirectValue::Scalar(value) => {
                tagged(e, scalar_tag, 1)?;
                e.field(1)?;
                self.value(value, e)
            }
            AbiDirectValue::DirectParts(parts) => {
                tagged(e, parts_tag, 2)?;
                e.field(1)?;
                self.value(parts.value(), e)?;
                e.field(2)?;
                e.array(parts.parts().len() as u64)?;
                for part in parts.parts() {
                    e.array(2)?;
                    e.unsigned(part.byte_offset)?;
                    crate::canonical_type::encode_type(
                        self.module,
                        &LirType::Ptr(part.pointer_kind),
                        e,
                    )?;
                }
                Ok(())
            }
        }
    }

    fn zst(&self, value: &AbiZst, e: &mut Encoder) -> Result {
        tagged(e, 1, 2)?;
        e.field(1)?;
        crate::canonical_type::encode_type(self.module, value.storage_type(), e)?;
        e.field(2)?;
        e.unsigned(value.layout().alignment().get())
    }

    fn value(&self, value: &AbiValue, e: &mut Encoder) -> Result {
        tagged(e, 1, 4)?;
        e.field(1)?;
        crate::canonical_type::encode_type(self.module, value.storage_type(), e)?;
        e.field(2)?;
        e.unsigned(value.layout().size().get())?;
        e.field(3)?;
        e.unsigned(value.layout().alignment().get())?;
        e.field(4)?;
        value.scan().encode(e)
    }
}

fn tagged(e: &mut Encoder, tag: u64, fields: u64) -> Result {
    e.map(fields + 1)?;
    e.field(0)?;
    e.unsigned(tag)
}
