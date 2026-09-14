use scoop_identity::DefinitionAtomRole;
use scoop_lir::{
    RefScan, StrongTypeDescriptorRefV1, StrongTypeDispatchCallableRefV1,
    StrongTypeRegistrationPlanV1, TypeInstanceShapeV1,
};
use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

use crate::link_object::callable_registrations::object_definition::{
    CanonicalAssociatedObjectAtomV1, CanonicalObjectRelocationV1,
    ObjectDefinitionFingerprintInputV1, ObjectDefinitionLeafWithAssociatedAtomsInputV1,
};
use crate::link_object::{LayoutFingerprintV1, ObjectDefinitionFingerprintV1};

const OBJECT_DEFINITION_DOMAIN: &str = "scoop-object-definition-v1";
const LAYOUT_DOMAIN: &str = "scoop-layout-v1";
const MANAGED_INSTANCE_LAYOUT_KIND: u32 = 2;
const TYPE_DESCRIPTOR_SEMANTIC_KIND: u32 = 1;

pub(super) fn descriptor_fingerprint(
    descriptor_bytes: &[u8],
    diagnostic_bytes: &[u8],
    plan: &StrongTypeRegistrationPlanV1,
) -> Result<ObjectDefinitionFingerprintV1, HashError> {
    let relocations = [CanonicalObjectRelocationV1::owning_associated_atom_offset(
        112,
        plan.diagnostic_atom(),
        DefinitionAtomRole::AddressTakenConstant,
        0,
    )];
    let associated_atoms = [CanonicalAssociatedObjectAtomV1 {
        atom: plan.diagnostic_atom(),
        role: DefinitionAtomRole::AddressTakenConstant,
        bytes: diagnostic_bytes,
        relocations: &[],
    }];
    domain_separated_runtime_hash(
        OBJECT_DEFINITION_DOMAIN,
        &TypeDescriptorObjectFingerprintInputV1 {
            object: ObjectDefinitionLeafWithAssociatedAtomsInputV1 {
                primary: ObjectDefinitionFingerprintInputV1 {
                    bytes: descriptor_bytes,
                    relocations: &relocations,
                    direct_inputs: &[],
                },
                associated_atoms: &associated_atoms,
            },
            plan,
        },
    )
    .map(|digest| ObjectDefinitionFingerprintV1::from_array(*digest.as_array()))
}

struct TypeDescriptorObjectFingerprintInputV1<'a> {
    object: ObjectDefinitionLeafWithAssociatedAtomsInputV1<'a>,
    plan: &'a StrongTypeRegistrationPlanV1,
}

impl RuntimeEncode for TypeDescriptorObjectFingerprintInputV1<'_> {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        self.object.runtime_encode(encoder)?;
        encoder.u32(TYPE_DESCRIPTOR_SEMANTIC_KIND)?;
        encode_descriptor_semantic(encoder, self.plan)
    }
}

pub(super) fn layout_fingerprint(
    plan: &StrongTypeRegistrationPlanV1,
) -> Result<LayoutFingerprintV1, HashError> {
    domain_separated_runtime_hash(
        LAYOUT_DOMAIN,
        &ManagedInstanceLayoutFingerprintInputV1(plan),
    )
    .map(|digest| LayoutFingerprintV1(*digest.as_array()))
}

struct ManagedInstanceLayoutFingerprintInputV1<'a>(&'a StrongTypeRegistrationPlanV1);

impl RuntimeEncode for ManagedInstanceLayoutFingerprintInputV1<'_> {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        let semantic = self.0.semantic();
        encoder.u32(MANAGED_INSTANCE_LAYOUT_KIND)?;
        encoder.fixed(semantic.instance_layout().as_array())?;
        encoder.fixed(semantic.exact_type().as_array())?;
        encoder.fixed(semantic.instance_scan().as_array())?;
        encode_instance_shape(encoder, semantic.instance_shape())
    }
}

fn encode_descriptor_semantic(
    encoder: &mut RuntimeEncoder,
    plan: &StrongTypeRegistrationPlanV1,
) -> Result<(), RuntimeEncodeError> {
    let semantic = plan.semantic();
    encoder.fixed(semantic.exact_type().as_array())?;
    encoder.u64(plan.runtime_type().get())?;
    encoder.fixed(semantic.instance_layout().as_array())?;
    encoder.fixed(semantic.instance_scan().as_array())?;
    encode_instance_shape(encoder, semantic.instance_shape())?;
    encoder.byte_span(semantic.diagnostic_name().as_bytes())?;
    encode_descriptor_ref(encoder, semantic.parent())?;
    encoder.fixed(semantic.vtable().table().as_array())?;
    encode_dispatch_slots(encoder, semantic.vtable().slots())?;
    encoder.sequence_length(semantic.itables().len())?;
    for itable in semantic.itables() {
        encoder.fixed(itable.table().as_array())?;
        encode_descriptor_ref(encoder, Some(itable.interface()))?;
        encode_dispatch_slots(encoder, itable.slots())?;
    }
    Ok(())
}

fn encode_instance_shape(
    encoder: &mut RuntimeEncoder,
    shape: &TypeInstanceShapeV1,
) -> Result<(), RuntimeEncodeError> {
    encoder.u32(shape.instance_kind().tag())?;
    encoder.u32(shape.inline_storage_kind().tag())?;
    encoder.u64(shape.minimum_size())?;
    encoder.u64(shape.instance_alignment())?;
    encoder.u64(shape.inline_offset())?;
    encoder.u64(shape.inline_size())?;
    encoder.u64(shape.inline_stride())?;
    encoder.u64(shape.inline_alignment())?;
    encode_scan(encoder, shape.object_scan())?;
    encode_scan(encoder, shape.inline_scan())
}

fn encode_scan(encoder: &mut RuntimeEncoder, scan: &RefScan) -> Result<(), RuntimeEncodeError> {
    match scan {
        RefScan::None => encoder.u32(0),
        RefScan::References(offsets) => {
            encoder.u32(1)?;
            encoder.sequence_length(offsets.len())?;
            for offset in offsets {
                encoder.u64(*offset)?;
            }
            Ok(())
        }
        RefScan::Sequence(parts) => {
            encoder.u32(2)?;
            encoder.sequence_length(parts.len())?;
            for part in parts {
                encode_scan(encoder, part)?;
            }
            Ok(())
        }
        RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            encoder.u32(3)?;
            encoder.u64(*length_offset)?;
            encoder.u64(*first_element_offset)?;
            encoder.u64(stride.get())?;
            encode_scan(encoder, element.as_ref_scan())
        }
    }
}

fn encode_descriptor_ref(
    encoder: &mut RuntimeEncoder,
    reference: Option<StrongTypeDescriptorRefV1>,
) -> Result<(), RuntimeEncodeError> {
    match reference {
        None => encoder.u32(0),
        Some(StrongTypeDescriptorRefV1::Local(exact_type)) => {
            encoder.u32(1)?;
            encoder.fixed(exact_type.as_array())
        }
        Some(StrongTypeDescriptorRefV1::CoreExternal(exact_type)) => {
            encoder.u32(2)?;
            encoder.fixed(exact_type.as_array())
        }
    }
}

fn encode_dispatch_slots(
    encoder: &mut RuntimeEncoder,
    slots: &[StrongTypeDispatchCallableRefV1],
) -> Result<(), RuntimeEncodeError> {
    encoder.sequence_length(slots.len())?;
    for slot in slots {
        match slot {
            StrongTypeDispatchCallableRefV1::Local(body) => {
                encoder.u32(1)?;
                encoder.fixed(body.as_array())?;
            }
            StrongTypeDispatchCallableRefV1::CoreExternal(body) => {
                encoder.u32(2)?;
                encoder.fixed(body.as_array())?;
            }
            StrongTypeDispatchCallableRefV1::Runtime(function) => {
                encoder.u32(3)?;
                encoder.u64(function.wire_family_tag())?;
                encoder.u64(function.wire_function_tag())?;
            }
        }
    }
    Ok(())
}
