use scoop_identity::DefinitionAtomRole;
use scoop_lir::{RefScan, StrongTypeRegistrationPlan, TypeInstanceShapeV1};
use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

use crate::link_object::callable_registrations::object_definition::{
    CanonicalAssociatedObjectAtomV1, CanonicalObjectRelocationV1,
    ObjectDefinitionFingerprintInputV1, ObjectDefinitionLeafWithAssociatedAtomsInputV1,
};
use crate::link_object::{LayoutFingerprintV1, ObjectDefinitionFingerprintV1};
use crate::link_object::{LinkDescriptorReference, LinkDispatchCallableReference};

const OBJECT_DEFINITION_DOMAIN: &str = "scoop-object-definition-v1";
const LAYOUT_DOMAIN: &str = "scoop-layout-v1";
const MANAGED_INSTANCE_LAYOUT_KIND: u32 = 2;
const TYPE_DESCRIPTOR_SEMANTIC_KIND: u32 = 1;

pub(super) fn descriptor_fingerprint<D, C>(
    descriptor_bytes: &[u8],
    diagnostic_bytes: &[u8],
    itable_directory: TypeDescriptorITableDirectoryFingerprintInputV1<'_>,
    plan: &StrongTypeRegistrationPlan<D, C>,
) -> Result<ObjectDefinitionFingerprintV1, HashError>
where
    D: LinkDescriptorReference,
    C: LinkDispatchCallableReference,
{
    let mut relocations = Vec::with_capacity(2);
    if let TypeDescriptorITableDirectoryFingerprintInputV1::Defined { atom, .. } = itable_directory
    {
        relocations.push(CanonicalObjectRelocationV1::owning_associated_atom_offset(
            96,
            atom,
            DefinitionAtomRole::RuntimeRecord,
            0,
        ));
    }
    relocations.push(CanonicalObjectRelocationV1::owning_associated_atom_offset(
        112,
        plan.diagnostic_atom(),
        DefinitionAtomRole::AddressTakenConstant,
        0,
    ));
    let directory_relocations = canonical_itable_directory_relocations(plan);
    let diagnostic_atom = CanonicalAssociatedObjectAtomV1 {
        atom: plan.diagnostic_atom(),
        role: DefinitionAtomRole::AddressTakenConstant,
        bytes: diagnostic_bytes,
        relocations: &[],
    };
    let directory_atom = match itable_directory {
        TypeDescriptorITableDirectoryFingerprintInputV1::Null => None,
        TypeDescriptorITableDirectoryFingerprintInputV1::Defined { atom, bytes } => {
            Some(CanonicalAssociatedObjectAtomV1 {
                atom,
                role: DefinitionAtomRole::RuntimeRecord,
                bytes,
                relocations: &directory_relocations,
            })
        }
    };
    let mut associated_atoms = Vec::with_capacity(2);
    if let Some(directory_atom) = directory_atom {
        associated_atoms.push(directory_atom);
    }
    associated_atoms.push(diagnostic_atom);
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

#[derive(Clone, Copy)]
pub(super) enum TypeDescriptorITableDirectoryFingerprintInputV1<'a> {
    Null,
    Defined {
        atom: scoop_identity::ObjectDefinitionAtomId,
        bytes: &'a [u8],
    },
}

fn canonical_itable_directory_relocations<D, C>(
    plan: &StrongTypeRegistrationPlan<D, C>,
) -> Vec<CanonicalObjectRelocationV1>
where
    D: LinkDescriptorReference,
{
    let mut relocations = Vec::new();
    for (index, itable) in plan.semantic().itables().iter().enumerate() {
        let base = u64::try_from(index).expect("itable index fits u64") * 16;
        relocations.push(itable.interface().canonical_relocation(base));
        if !itable.slots().is_empty() {
            relocations.push(CanonicalObjectRelocationV1::dispatch_table(
                base + 8,
                itable.table(),
            ));
        }
    }
    relocations
}

struct TypeDescriptorObjectFingerprintInputV1<'a, D, C> {
    object: ObjectDefinitionLeafWithAssociatedAtomsInputV1<'a>,
    plan: &'a StrongTypeRegistrationPlan<D, C>,
}

impl<D, C> RuntimeEncode for TypeDescriptorObjectFingerprintInputV1<'_, D, C>
where
    D: LinkDescriptorReference,
    C: LinkDispatchCallableReference,
{
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        self.object.runtime_encode(encoder)?;
        encoder.u32(TYPE_DESCRIPTOR_SEMANTIC_KIND)?;
        encode_descriptor_semantic(encoder, self.plan)
    }
}

pub(super) fn layout_fingerprint<D: Copy, C>(
    plan: &StrongTypeRegistrationPlan<D, C>,
) -> Result<LayoutFingerprintV1, HashError> {
    domain_separated_runtime_hash(
        LAYOUT_DOMAIN,
        &ManagedInstanceLayoutFingerprintInputV1(plan),
    )
    .map(|digest| LayoutFingerprintV1(*digest.as_array()))
}

struct ManagedInstanceLayoutFingerprintInputV1<'a, D, C>(&'a StrongTypeRegistrationPlan<D, C>);

impl<D: Copy, C> RuntimeEncode for ManagedInstanceLayoutFingerprintInputV1<'_, D, C> {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        let semantic = self.0.semantic();
        encoder.u32(MANAGED_INSTANCE_LAYOUT_KIND)?;
        encoder.fixed(semantic.instance_layout().as_array())?;
        encoder.fixed(semantic.exact_type().as_array())?;
        encoder.fixed(semantic.instance_scan().as_array())?;
        encode_instance_shape(encoder, semantic.instance_shape())
    }
}

fn encode_descriptor_semantic<D, C>(
    encoder: &mut RuntimeEncoder,
    plan: &StrongTypeRegistrationPlan<D, C>,
) -> Result<(), RuntimeEncodeError>
where
    D: LinkDescriptorReference,
    C: LinkDispatchCallableReference,
{
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

fn encode_descriptor_ref<D: LinkDescriptorReference>(
    encoder: &mut RuntimeEncoder,
    reference: Option<D>,
) -> Result<(), RuntimeEncodeError> {
    match reference {
        None => encoder.u32(0),
        Some(reference) => reference.runtime_encode(encoder),
    }
}

fn encode_dispatch_slots<C: LinkDispatchCallableReference>(
    encoder: &mut RuntimeEncoder,
    slots: &[C],
) -> Result<(), RuntimeEncodeError> {
    encoder.sequence_length(slots.len())?;
    for slot in slots {
        slot.runtime_encode(encoder)?;
    }
    Ok(())
}
