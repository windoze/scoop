use super::*;

impl WireEncode for BackendProfileContract {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if self.machine_frame.is_some() { 28 } else { 26 })?;
        encode_pair(
            encoder,
            1,
            u64::from(self.llvm_api_version[0]),
            u64::from(self.llvm_api_version[1]),
        )?;
        encoder.field(2)?;
        encoder.map(3)?;
        for (index, value) in self.inkwell_binding.iter().enumerate() {
            encode_unsigned_field(encoder, index as u32 + 1, u64::from(*value))?;
        }
        encoder.field(3)?;
        encoder.text(self.cpu)?;
        encoder.field(4)?;
        encoder.text(self.features)?;
        for field in 5..=10 {
            encode_unsigned_field(encoder, field, 1)?;
        }
        encode_unsigned_field(encoder, 11, 3)?;
        for field in 12..=18 {
            encode_unsigned_field(encoder, field, 1)?;
        }
        encode_unsigned_field(encoder, 19, u64::from(self.llvm_target_backend))?;
        encode_unsigned_field(encoder, 20, 1)?;
        encode_unsigned_field(encoder, 21, 1)?;
        encode_unsigned_field(encoder, 22, 2)?;
        encoder.field(23)?;
        encoder.map(if self.lsda.omitted_type_table.is_some() {
            4
        } else {
            3
        })?;
        encode_unsigned_field(encoder, 1, u64::from(self.lsda.lp_start))?;
        encode_unsigned_field(encoder, 2, u64::from(self.lsda.type_table))?;
        encode_unsigned_field(encoder, 3, u64::from(self.lsda.call_site))?;
        if let Some(encoding) = self.lsda.omitted_type_table {
            encode_unsigned_field(encoder, 4, u64::from(encoding))?;
        }
        encode_unsigned_field(encoder, 24, u64::from(self.unwind_provider))?;
        encode_unsigned_field(
            encoder,
            25,
            match self.artifact_inspection {
                crate::NativeObjectFormat::MachO64 => 1,
                crate::NativeObjectFormat::Elf64 => 2,
            },
        )?;
        if let Some(frame) = self.machine_frame {
            encode_unsigned_field(encoder, 26, 1)?;
            encoder.field(27)?;
            encoder.map(4)?;
            for (field, value) in [
                (1, u64::from(frame.stack_pointer_register)),
                (2, u64::from(frame.frame_pointer_register)),
                (3, u64::from(frame.root_slot_bytes)),
                (4, u64::from(frame.frame_record_bytes)),
            ] {
                encode_unsigned_field(encoder, field, value)?;
            }
        }
        // Qualified ordinary passes and final physical GC/EH emission plans.
        encode_unsigned_field(encoder, 28, 1)?;
        Ok(())
    }
}

fn encode_pair(
    encoder: &mut Encoder,
    field: u32,
    first: u64,
    second: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.map(2)?;
    encode_unsigned_field(encoder, 1, first)?;
    encode_unsigned_field(encoder, 2, second)
}

fn encode_unsigned_field(
    encoder: &mut Encoder,
    field: u32,
    value: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.unsigned(value)
}
