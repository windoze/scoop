use super::{ShapeIdentity, projection::ShapeContent};
use crate::*;
use scoop_wire::{Encoder, WireEncode, cbor::EncodeError};

type Result = std::result::Result<(), EncodeError>;

pub(super) struct ShapeProjection<'a> {
    pub(super) module: &'a Module,
    pub(super) shape: ShapeContent<'a>,
    pub(super) abi: bool,
}

pub(super) struct AbiProjection<'a> {
    pub(super) module: &'a Module,
    pub(super) shape: ShapeContent<'a>,
    pub(super) identity: ShapeIdentity,
}

impl WireEncode for AbiProjection<'_> {
    fn encode(&self, e: &mut Encoder) -> Result {
        e.map(4)?;
        e.field(1)?;
        self.identity.group.encode(e)?;
        e.field(2)?;
        self.identity.member.encode(e)?;
        e.field(3)?;
        self.identity.role.encode(e)?;
        e.field(4)?;
        ShapeProjection {
            module: self.module,
            shape: self.shape,
            abi: true,
        }
        .encode(e)
    }
}

impl WireEncode for ShapeProjection<'_> {
    fn encode(&self, e: &mut Encoder) -> Result {
        match self.shape {
            ShapeContent::Fixed(layout) => {
                tagged(e, 1, 8)?;
                e.field(1)?;
                layout.identity.layout_record().id().encode(e)?;
                e.field(2)?;
                e.unsigned(layout.size)?;
                e.field(3)?;
                e.unsigned(layout.align)?;
                e.field(4)?;
                e.array(layout.fields.len() as u64)?;
                for field in &layout.fields {
                    e.array(2)?;
                    e.unsigned(field.offset)?;
                    e.unsigned(field.access_align)?;
                }
                e.field(5)?;
                e.array(u64::from(layout.c_layout.is_some()))?;
                if let Some(contract) = layout.c_layout {
                    e.array(2)?;
                    e.unsigned(contract.aligned.bytes().unwrap_or(0))?;
                    e.unsigned(contract.packed.bytes().unwrap_or(0))?;
                }
                e.field(6)?;
                e.unsigned(u64::from(layout.interior_mutable))?;
                e.field(7)?;
                self.layout_kind(&layout.kind, e)
            }
            ShapeContent::Array(array) => {
                tagged(e, 2, 7)?;
                e.field(1)?;
                array.identity.layout_record().id().encode(e)?;
                e.field(2)?;
                e.unsigned(match array.kind {
                    ArrayKind::Immutable => 1,
                    ArrayKind::Mutable => 2,
                })?;
                e.field(3)?;
                array.element_exact.encode(e)?;
                e.field(4)?;
                crate::canonical_type::encode_type(self.module, &array.element, e)?;
                e.field(5)?;
                array.layout.instance().encode(e)?;
                e.field(6)?;
                e.unsigned(array.layout.maximum_count())
            }
            ShapeContent::Instance(descriptor) => {
                tagged(e, 3, 3)?;
                e.field(1)?;
                descriptor.instance_layout.layout_record().id().encode(e)?;
                e.field(2)?;
                descriptor.instance_shape.encode(e)
            }
            ShapeContent::Scan(scan) => {
                tagged(e, 4, 2)?;
                e.field(1)?;
                scan.encode(e)
            }
            ShapeContent::Descriptor(descriptor) => self.descriptor(descriptor, e),
            ShapeContent::Dispatch(table, slots) => {
                tagged(e, 6, 3)?;
                e.field(1)?;
                table.encode(e)?;
                e.field(2)?;
                e.array(slots.len() as u64)?;
                for slot in slots {
                    self.callable(slot.callable, e)?;
                }
                Ok(())
            }
            ShapeContent::Immortal(plan, value) => {
                tagged(e, 7, if self.abi { 5 } else { 6 })?;
                e.field(1)?;
                plan.object().encode(e)?;
                e.field(2)?;
                plan.type_registration().encode(e)?;
                e.field(3)?;
                e.unsigned(plan.object_size())?;
                e.field(4)?;
                e.unsigned(plan.required_alignment())?;
                if !self.abi {
                    e.field(5)?;
                    e.bytes(value.as_bytes())?;
                }
                Ok(())
            }
            ShapeContent::Storage(plan) => {
                if self.abi {
                    tagged(e, 8, 6)?;
                    e.field(1)?;
                    plan.storage().encode(e)?;
                    e.field(2)?;
                    plan.layout().encode(e)?;
                    e.field(3)?;
                    e.unsigned(plan.byte_size())?;
                    e.field(4)?;
                    e.unsigned(plan.allocation_extent())?;
                    e.field(5)?;
                    e.unsigned(plan.required_alignment())
                } else {
                    tagged(e, 8, 2)?;
                    e.field(1)?;
                    plan.canonical_projection().encode(e)
                }
            }
            ShapeContent::InitializationCell(unit) => {
                tagged(e, 9, if self.abi { 4 } else { 5 })?;
                e.field(1)?;
                unit.encode(e)?;
                e.field(2)?;
                e.unsigned(16)?;
                e.field(3)?;
                e.unsigned(8)?;
                if !self.abi {
                    e.field(4)?;
                    e.bytes(&[0; 16])?;
                }
                Ok(())
            }
            ShapeContent::InitializationDescriptor(unit) => {
                tagged(e, 10, if self.abi { 4 } else { 10 })?;
                e.field(1)?;
                unit.identity.id().encode(e)?;
                e.field(2)?;
                e.unsigned(88)?;
                e.field(3)?;
                e.unsigned(8)?;
                if !self.abi {
                    e.field(4)?;
                    e.unsigned(match unit.schedule {
                        InitializationSchedule::EagerStartup => 0,
                        InitializationSchedule::LazyAccess => 1,
                    })?;
                    for (field, global) in [(5, unit.kind.storage()), (6, unit.failure_root)] {
                        let GlobalInit::Storage { identity, .. } =
                            &self.module.globals[global].init
                        else {
                            unreachable!("a complete initialization unit has local static storage")
                        };
                        e.field(field)?;
                        identity.identity_record().id().encode(e)?;
                    }
                    e.field(7)?;
                    self.callable(CallableRef::Local(unit.initializer.declaration()), e)?;
                    e.field(8)?;
                    e.text(&unit.display_name)?;
                    e.field(9)?;
                    self.callable(CallableRef::Local(unit.ensure.declaration()), e)?;
                }
                Ok(())
            }
        }
    }
}

impl ShapeProjection<'_> {
    fn layout_kind(&self, kind: &LayoutKind, e: &mut Encoder) -> Result {
        match kind {
            LayoutKind::Plain { scan } => {
                tagged(e, 1, 2)?;
                e.field(1)?;
                scan.encode(e)
            }
            LayoutKind::Enum { scan } => {
                tagged(e, 2, 2)?;
                e.field(1)?;
                scan.encode(e)
            }
            LayoutKind::Intrinsic(value) => {
                tagged(e, 3, 2)?;
                e.field(1)?;
                match value {
                    IntrinsicTypeRepresentation::Integer(kind) => {
                        tagged(e, 1, 2)?;
                        e.field(1)?;
                        crate::canonical_type::encode_integer(*kind, e)
                    }
                    IntrinsicTypeRepresentation::Boolean => tagged(e, 2, 1),
                    IntrinsicTypeRepresentation::String => tagged(e, 3, 1),
                    IntrinsicTypeRepresentation::Ptr { pointee } => {
                        tagged(e, 4, 2)?;
                        e.field(1)?;
                        match pointee {
                            LirDataPointee::OpaqueVoid => e.array(0),
                            LirDataPointee::Value(ty) => {
                                e.array(1)?;
                                crate::canonical_type::encode_type(self.module, ty, e)
                            }
                        }
                    }
                    IntrinsicTypeRepresentation::FunPtr { signature } => {
                        tagged(e, 5, 3)?;
                        e.field(1)?;
                        e.array(signature.params.len() as u64)?;
                        for ty in &signature.params {
                            crate::canonical_type::encode_type(self.module, ty, e)?;
                        }
                        e.field(2)?;
                        match &signature.return_type {
                            LirReturnType::Void => e.array(0),
                            LirReturnType::Value(ty) => {
                                e.array(1)?;
                                crate::canonical_type::encode_type(self.module, ty, e)
                            }
                        }
                    }
                }
            }
        }
    }

    fn descriptor(&self, descriptor: &TypeDescriptor, e: &mut Encoder) -> Result {
        tagged(e, 5, if self.abi { 9 } else { 10 })?;
        e.field(1)?;
        descriptor.identity.exact_type().encode(e)?;
        e.field(2)?;
        descriptor
            .identity
            .runtime_type()
            .runtime_type()
            .encode(e)?;
        e.field(3)?;
        descriptor.instance_layout.layout_record().id().encode(e)?;
        e.field(4)?;
        descriptor.instance_shape.encode(e)?;
        e.field(5)?;
        match descriptor.inline_scan {
            TypeDescriptorInlineScanV1::Null => e.array(0)?,
            TypeDescriptorInlineScanV1::Defined(scan) => {
                e.array(1)?;
                scan.encode(e)?;
            }
        }
        e.field(6)?;
        e.array(u64::from(descriptor.parent.is_some()))?;
        if let Some(parent) = descriptor.parent {
            self.descriptor_reference(parent, e)?;
        }
        e.field(7)?;
        e.array(2)?;
        descriptor.vtable.identity_record().id().encode(e)?;
        e.array(descriptor.itables.len() as u64)?;
        for table in &descriptor.itables {
            e.array(2)?;
            self.descriptor_reference(table.interface(), e)?;
            table.identity_record().id().encode(e)?;
        }
        e.field(8)?;
        descriptor.relations.encode_with(e, |reference, e| {
            e.array(u64::from(reference.is_some()))?;
            if let Some(reference) = reference {
                self.descriptor_reference(*reference, e)?;
            }
            Ok(())
        })?;
        if !self.abi {
            e.field(9)?;
            e.text(&descriptor.diagnostic_name)?;
        }
        Ok(())
    }

    fn descriptor_reference(&self, reference: TypeDescriptorRef, e: &mut Encoder) -> Result {
        match reference {
            TypeDescriptorRef::Local(id) => self.module.meta.type_descriptors[id]
                .identity
                .exact_type()
                .encode(e),
            TypeDescriptorRef::External(id) => self.module.meta.external_type_descriptors[id]
                .target()
                .encode(e),
        }
    }

    fn callable(&self, reference: CallableRef, e: &mut Encoder) -> Result {
        match reference {
            CallableRef::Local(id) => {
                tagged(e, 1, 2)?;
                e.field(1)?;
                self.module.functions[id.into_u32() as usize]
                    .callable_body
                    .id()
                    .encode(e)
            }
            CallableRef::External(id) => {
                tagged(e, 1, 2)?;
                e.field(1)?;
                self.module.meta.external_callables[id].body().encode(e)
            }
            CallableRef::Runtime(function) => {
                tagged(e, 2, 3)?;
                e.field(1)?;
                e.unsigned(function.wire_family_tag())?;
                e.field(2)?;
                e.unsigned(function.wire_function_tag())
            }
        }
    }
}

fn tagged(e: &mut Encoder, tag: u64, fields: u64) -> Result {
    e.map(fields)?;
    e.field(0)?;
    e.unsigned(tag)
}
