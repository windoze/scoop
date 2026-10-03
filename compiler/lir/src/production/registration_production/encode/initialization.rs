use super::*;

impl<D: WireEncode> WireEncode for crate::StrongInitializationUnitRegistrationPlan<D> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let semantic = self.semantic();
        encoder.map(24)?;
        super::projections::encode_unit_semantic_fields(encoder, semantic)?;
        encode_field(encoder, 9, &self.registration_symbol())?;
        encode_field(encoder, 10, &self.registration_definition_plan())?;
        encode_field(encoder, 11, &self.registration_primary_atom())?;
        encode_field(encoder, 12, &self.cell_symbol())?;
        encode_field(encoder, 13, &self.cell_definition_plan())?;
        encode_field(encoder, 14, &self.cell_primary_atom())?;
        encode_field(encoder, 18, &self.diagnostic_atom())?;
        encoder.field(19)?;
        encode_initialization_storage_ref(encoder, self.storage())?;
        encoder.field(20)?;
        encode_initialization_storage_ref(encoder, self.failure_root())?;
        encoder.field(21)?;
        encode_initialization_callable_ref(encoder, self.initializer())?;
        encoder.field(22)?;
        encode_initialization_callable_ref(encoder, self.ensure())?;
        encoder.field(23)?;
        encode_initialization_registration_schedule(encoder, self.schedule())?;
        encode_field(encoder, 24, &self.registration_object_node())?;
        encode_field(encoder, 25, &self.cell_definition_node())?;
        encode_field(encoder, 27, &self.registration_fingerprint_node())?;
        encode_field(encoder, 28, &self.registration_definition_patch())
    }
}

pub(super) fn encode_static_initial_state(
    encoder: &mut Encoder,
    state: &StrongStaticStorageInitialStatePlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match state {
        StrongStaticStorageInitialStatePlanV1::ZeroedForRuntimeUnit => encode_empty_sum(encoder, 1),
        StrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
            initial_template,
            immortal_relocations,
        } => {
            encoder.map(3)?;
            encode_unsigned_field(encoder, 0, 2)?;
            encoder.field(1)?;
            encoder.bytes(initial_template)?;
            encoder.field(2)?;
            encoder.array(immortal_relocations.len() as u64)?;
            for relocation in immortal_relocations {
                encoder.map(2)?;
                encode_unsigned_field(encoder, 1, relocation.pointer_offset())?;
                encode_field(encoder, 2, &relocation.target())?;
            }
            Ok(())
        }
    }
}

pub(super) fn encode_static_initial_artifacts(
    encoder: &mut Encoder,
    artifacts: StrongStaticStorageInitialArtifactPlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match artifacts {
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit => {
            encode_empty_sum(encoder, 1)
        }
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            template_atom,
            relocation_table,
        } => {
            encoder.map(3)?;
            encode_unsigned_field(encoder, 0, 2)?;
            encode_field(encoder, 1, &template_atom)?;
            encoder.field(2)?;
            match relocation_table {
                StaticStorageRelocationTableArtifactV1::SharedEmptySentinel => {
                    encode_empty_sum(encoder, 1)
                }
                StaticStorageRelocationTableArtifactV1::Defined { atom } => {
                    encode_value_sum(encoder, 2, &atom)
                }
            }
        }
    }
}

pub(super) fn encode_initialization_semantic_schedule(
    encoder: &mut Encoder,
    schedule: crate::StrongInitializationSchedulePlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match schedule {
        crate::StrongInitializationSchedulePlanV1::EagerStartup { gateway } => {
            encode_value_sum(encoder, 1, &gateway)
        }
        crate::StrongInitializationSchedulePlanV1::LazyAccess => encode_empty_sum(encoder, 2),
    }
}

fn encode_initialization_storage_ref(
    encoder: &mut Encoder,
    reference: StrongInitializationStaticStorageRefPlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(6)?;
    encode_field(encoder, 1, &reference.storage())?;
    encode_field(encoder, 2, &reference.storage_symbol())?;
    encode_field(encoder, 3, &reference.registration_symbol())?;
    encode_field(encoder, 4, &reference.registration_definition_plan())?;
    encode_field(encoder, 5, &reference.registration_primary_atom())?;
    encode_field(encoder, 6, &reference.registration_fingerprint_node())
}

fn encode_initialization_callable_ref(
    encoder: &mut Encoder,
    reference: StrongInitializationCallableRefPlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(9)?;
    encode_field(encoder, 1, &reference.body())?;
    encode_field(encoder, 2, &reference.entry_symbol())?;
    encode_field(encoder, 3, &reference.registration_symbol())?;
    encode_field(encoder, 4, &reference.body_definition_plan())?;
    encode_field(encoder, 5, &reference.body_primary_atom())?;
    encode_field(encoder, 6, &reference.body_definition_node())?;
    encode_field(encoder, 7, &reference.registration_definition_plan())?;
    encode_field(encoder, 8, &reference.registration_primary_atom())?;
    encode_field(encoder, 9, &reference.registration_fingerprint_node())
}

fn encode_initialization_registration_schedule(
    encoder: &mut Encoder,
    schedule: &StrongInitializationRegistrationSchedulePlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match schedule {
        StrongInitializationRegistrationSchedulePlanV1::EagerStartup {
            gateway,
            gateway_definition_patch,
        } => {
            encoder.map(3)?;
            encode_unsigned_field(encoder, 0, 1)?;
            encoder.field(1)?;
            encode_initialization_callable_ref(encoder, **gateway)?;
            encode_field(encoder, 2, gateway_definition_patch)
        }
        StrongInitializationRegistrationSchedulePlanV1::LazyAccess => encode_empty_sum(encoder, 2),
    }
}
