use super::*;

mod initialization;
mod types;
use initialization::{encode_static_initial_artifacts, encode_static_initial_state};
mod projections;
use projections::encode_static_semantic_fields;
pub use projections::*;

impl<D: crate::StrongDescriptorReference, C: Clone + WireEncode, I: WireEncode> WireEncode
    for StrongRegistrationProductionSurface<D, C, I>
{
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encode_field(encoder, 1, &self.identities)?;
        encode_array_field(encoder, 2, self.safepoints.registrations())?;
        encode_array_field(encoder, 3, self.callables.registrations())?;
        encode_array_field(encoder, 4, self.types.registrations())?;
        encode_array_field(encoder, 5, self.immortal_objects.registrations())?;
        encode_array_field(encoder, 6, self.static_storages.registrations())?;
        encode_array_field(encoder, 7, self.initialization_units.registrations())?;
        encoder.field(8)?;
        encode_callable_runtime_scans(encoder, self.callables.runtime_scans())
    }
}

impl WireEncode for StrongSafepointRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encode_field(encoder, 1, &self.site())?;
        encode_field(encoder, 2, &self.safepoint())?;
        encode_field(encoder, 3, &self.owner())?;
        encode_field(encoder, 4, &self.role())?;
        encode_unsigned_field(encoder, 5, u64::from(self.root_pair_count()))?;
        encode_field(encoder, 6, &self.symbol())?;
        encode_field(encoder, 7, &self.definition_plan())?;
        encode_field(encoder, 8, &self.primary_atom())?;
        encode_field(encoder, 9, &self.registration_fingerprint_node())?;
        encode_field(encoder, 10, &self.normalized_stackmap_fingerprint_node())?;
        encode_field(encoder, 11, &self.registration_definition_patch())?;
        encode_field(encoder, 12, &self.normalized_stackmap_patch())
    }
}

impl WireEncode for StrongCallableRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encode_field(encoder, 1, &self.body())?;
        encode_field(encoder, 2, &self.symbol())?;
        encode_field(encoder, 3, &self.definition_plan())?;
        encode_field(encoder, 4, &self.primary_atom())?;
        encode_field(encoder, 5, &self.entry_symbol())?;
        encode_field(encoder, 6, &self.body_definition_plan())?;
        encode_field(encoder, 7, &self.body_primary_atom())?;
        encode_field(encoder, 8, &self.registration_object_node())?;
        encode_field(encoder, 9, &self.body_definition_node())?;
        encode_field(encoder, 10, &self.registration_fingerprint_node())?;
        encode_field(encoder, 11, &self.registration_definition_patch())?;
        encode_field(encoder, 12, &self.body_definition_patch())
    }
}

fn encode_callable_runtime_scans(
    encoder: &mut Encoder,
    plans: &StrongCallableRuntimeScanPlanSetV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(plans.callables().len() as u64)?;
    for callable in plans.callables() {
        encoder.map(2)?;
        encode_field(encoder, 1, &callable.body())?;
        encoder.field(2)?;
        encoder.array(callable.atoms().len() as u64)?;
        for atom in callable.atoms() {
            encoder.map(2)?;
            encode_field(encoder, 1, &atom.atom())?;
            encoder.field(2)?;
            encode_ref_scan(encoder, atom.scan())?;
        }
    }
    Ok(())
}

impl WireEncode for StrongImmortalObjectRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(15)?;
        encode_field(encoder, 1, &self.object())?;
        encode_field(encoder, 2, &self.object_symbol())?;
        encode_unsigned_field(encoder, 3, self.object_size())?;
        encode_unsigned_field(encoder, 4, self.required_alignment())?;
        encoder.field(5)?;
        encode_type_registration_ref(encoder, self.semantic().type_registration_ref())?;
        encode_field(encoder, 6, &self.registration_symbol())?;
        encode_field(encoder, 7, &self.registration_definition_plan())?;
        encode_field(encoder, 8, &self.registration_primary_atom())?;
        encode_field(encoder, 9, &self.object_definition_plan())?;
        encode_field(encoder, 10, &self.object_primary_atom())?;
        encode_field(encoder, 11, &self.type_registration_symbol())?;
        encode_field(encoder, 12, &self.registration_object_node())?;
        encode_field(encoder, 13, &self.object_definition_node())?;
        encode_field(encoder, 14, &self.registration_fingerprint_node())?;
        encode_field(encoder, 15, &self.registration_definition_patch())
    }
}

impl WireEncode for StrongStaticStorageRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let semantic = self.semantic();
        encoder.map(32)?;
        encode_static_semantic_fields(encoder, semantic)?;
        encode_field(encoder, 11, &self.registration_symbol())?;
        encode_field(encoder, 12, &self.registration_definition_plan())?;
        encode_field(encoder, 13, &self.registration_primary_atom())?;
        encode_field(encoder, 14, &self.storage_definition_plan())?;
        encode_field(encoder, 15, &self.storage_primary_atom())?;
        encoder.field(16)?;
        encode_static_initial_artifacts(encoder, self.initial_artifacts())?;
        encode_array_field(encoder, 17, self.immortal_registration_symbols())?;
        encode_field(encoder, 18, &self.layout_symbol())?;
        encode_field(encoder, 19, &self.layout_definition_plan())?;
        encode_field(encoder, 20, &self.layout_primary_atom())?;
        encode_field(encoder, 21, &self.scan_symbol())?;
        encode_field(encoder, 22, &self.scan_definition_plan())?;
        encode_field(encoder, 23, &self.scan_primary_atom())?;
        encode_field(encoder, 24, &self.registration_object_node())?;
        encode_field(encoder, 25, &self.storage_definition_node())?;
        encode_field(encoder, 26, &self.layout_fingerprint_node())?;
        encode_field(encoder, 27, &self.scan_fingerprint_node())?;
        encode_field(encoder, 28, &self.registration_fingerprint_node())?;
        encode_field(encoder, 29, &self.registration_definition_patch())?;
        encode_field(encoder, 30, &self.layout_fingerprint_patch())?;
        encode_field(encoder, 31, &self.scan_fingerprint_patch())?;
        encode_field(encoder, 32, &semantic.layout_provider())
    }
}

fn encode_type_registration_ref(
    encoder: &mut Encoder,
    registration: ImmortalObjectTypeRegistrationRefV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match registration {
        ImmortalObjectTypeRegistrationRefV1::Local(exact) => {
            crate::StrongTypeDescriptorRefV2::Local(exact)
        }
        ImmortalObjectTypeRegistrationRefV1::DependencyExternal { provider, exact } => {
            crate::StrongTypeDescriptorRefV2::DependencyExternal { provider, exact }
        }
    }
    .encode(encoder)
}

pub(super) fn encode_ref_scan(
    encoder: &mut Encoder,
    scan: &RefScan,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match scan {
        RefScan::None => encode_empty_sum(encoder, 1),
        RefScan::References(offsets) => {
            encoder.map(2)?;
            encode_unsigned_field(encoder, 0, 2)?;
            encoder.field(1)?;
            encoder.array(offsets.len() as u64)?;
            for offset in offsets {
                encoder.unsigned(*offset)?;
            }
            Ok(())
        }
        RefScan::Sequence(parts) => {
            encoder.map(2)?;
            encode_unsigned_field(encoder, 0, 3)?;
            encoder.field(1)?;
            encoder.array(parts.len() as u64)?;
            for part in parts {
                encode_ref_scan(encoder, part)?;
            }
            Ok(())
        }
        RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            encoder.map(5)?;
            encode_unsigned_field(encoder, 0, 4)?;
            encode_unsigned_field(encoder, 1, *length_offset)?;
            encode_unsigned_field(encoder, 2, *first_element_offset)?;
            encode_unsigned_field(encoder, 3, stride.get())?;
            encoder.field(4)?;
            encode_ref_scan(encoder, element.as_ref_scan())
        }
    }
}

impl WireEncode for RefScan {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_ref_scan(encoder, self)
    }
}

fn encode_field(
    encoder: &mut Encoder,
    field: u32,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    value.encode(encoder)
}

fn encode_unsigned_field(
    encoder: &mut Encoder,
    field: u32,
    value: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.unsigned(value)
}

fn encode_array_field<T: WireEncode>(
    encoder: &mut Encoder,
    field: u32,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_unsigned_field(encoder, 0, tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_unsigned_field(encoder, 0, tag)?;
    encode_field(encoder, 1, value)
}
