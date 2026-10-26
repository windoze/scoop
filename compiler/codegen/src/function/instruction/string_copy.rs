//! Copy stable raw bytes or relocated object bytes into one exact allocation.

use super::*;
use crate::function::gc::AllocationLayout;
use scoop_lir::{StringCopySource as Source, StringCopyTarget as Target};

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_string_byte(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let (string, index) = match instruction {
            Instruction::StringByteLoad { string, index, .. }
            | Instruction::StringByteStore { string, index, .. } => (*string, *index),
            _ => unreachable!("String byte instruction dispatch is exhaustive"),
        };
        let string = self.value(string)?.into_pointer_value();
        let index = self.value(index)?.into_int_value();
        let bytes = self.byte_gep(string, 24, "string.bytes")?;
        // SAFETY: core established the byte range and keeps this managed
        // operand live. No safepoint separates the address from its use.
        let address = unsafe {
            self.builder
                .build_gep(self.context.i8_type(), bytes, &[index], "string.byte_ptr")
        }
        .map_err(copy_error)?;
        match instruction {
            Instruction::StringByteLoad { out, .. } => {
                let byte = self
                    .builder
                    .build_load(self.context.i8_type(), address, "string.byte")
                    .map_err(copy_error)?;
                self.temps.insert(*out, byte);
            }
            Instruction::StringByteStore { value, .. } => {
                let value = self.value(*value)?;
                self.builder
                    .build_store(address, value)
                    .map_err(copy_error)?;
            }
            _ => unreachable!("String byte instruction dispatch is exhaustive"),
        }
        Ok(())
    }

    pub(super) fn emit_string_copy(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let Instruction::StringCopy {
            out,
            source,
            target,
            overflow_message,
            safepoint,
            live,
        } = instruction
        else {
            unreachable!("String copy instruction dispatch is exhaustive");
        };
        let context = self.context;
        let builder = self.builder;
        let word = context.i64_type();
        let (length, offset) = match *source {
            Source::Zeroed(length) => (self.value(length)?.into_int_value(), word.const_zero()),
            Source::Utf8 { length, .. } => {
                (self.value(length)?.into_int_value(), word.const_zero())
            }
            Source::Slice { start, end, .. } => {
                let start = self.value(start)?.into_int_value();
                let end = self.value(end)?.into_int_value();
                // Core's scalar-range check established this immutable byte range.
                (
                    builder
                        .build_int_sub(end, start, "copy.length")
                        .map_err(copy_error)?,
                    builder
                        .build_int_add(start, word.const_int(24, false), "copy.start")
                        .map_err(copy_error)?,
                )
            }
            Source::Bytes { array, array_type } => {
                let offset = self
                    .array_type(array_type)
                    .0
                    .layout
                    .instance()
                    .inline_offset();
                (self.copy_byte_count(array)?, word.const_int(offset, false))
            }
            Source::String(string) => (self.copy_byte_count(string)?, word.const_int(24, false)),
        };
        let (descriptor, bytes, layout, destination_offset) = match *target {
            Target::String(descriptor) => {
                // This unsigned limit also rejects negative raw-buffer lengths.
                let invalid = builder
                    .build_int_compare(
                        IntPredicate::UGT,
                        length,
                        word.const_int(i64::MAX as u64 - 31, false),
                        "copy.overflow",
                    )
                    .map_err(copy_error)?;
                self.array_size_check(invalid, *overflow_message, "copy.size.ok")?;
                let bytes = builder
                    .build_int_add(length, word.const_int(24, false), "copy.bytes")
                    .map_err(copy_error)?;
                (
                    self.value(Value::TypeDescriptor(descriptor))?
                        .into_pointer_value(),
                    bytes,
                    AllocationLayout::string(),
                    24,
                )
            }
            Target::Bytes(array_type) => {
                let (metadata, descriptor) = self.array_type(array_type);
                let layout = metadata.layout.clone();
                let offset = layout.instance().inline_offset();
                let bytes =
                    self.checked_array_allocation_size(&layout, length, *overflow_message)?;
                (
                    descriptor.as_pointer_value(),
                    bytes,
                    AllocationLayout::array(&layout),
                    offset,
                )
            }
        };
        let safepoint = self.safepoint_id(*safepoint);
        let live = self.materialize_statepoint_live(live, safepoint)?;
        let result =
            self.managed_alloc_value(descriptor, bytes, Some(layout), safepoint, live, None)?;
        let result = result.object;
        let count = self.byte_gep(result, 16, "copy.result_count")?;
        builder.build_store(count, length).map_err(copy_error)?;

        if matches!(source, Source::Zeroed(_)) {
            // The shared allocation path already clears the complete object.
            self.temps.insert(*out, result.into());
            return Ok(());
        }

        // Materialize managed source addresses only after the collecting allocation.
        let source = match *source {
            Source::Zeroed(_) => unreachable!("zero initialization completed above"),
            Source::Utf8 { pointer, .. } => self.value(pointer)?,
            Source::Slice { string, .. } | Source::String(string) => self.value(string)?,
            Source::Bytes { array, .. } => self.value(array)?,
        }
        .into_pointer_value();
        // SAFETY: the typed copy contract bounds this range; raw memory stays stable.
        let source =
            unsafe { builder.build_gep(context.i8_type(), source, &[offset], "copy.source") }
                .map_err(copy_error)?;
        let destination = self.byte_gep(result, destination_offset, "copy.destination")?;
        builder
            .build_memcpy(destination, 1, source, 1, length)
            .map_err(copy_error)?;
        self.temps.insert(*out, result.into());
        Ok(())
    }

    fn copy_byte_count(&mut self, object: Value) -> Result<IntValue<'ctx>, CodegenError> {
        let object = self.value(object)?.into_pointer_value();
        let count = self.byte_gep(object, 16, "copy.count_ptr")?;
        self.builder
            .build_load(self.context.i64_type(), count, "copy.count")
            .map(BasicValueEnum::into_int_value)
            .map_err(copy_error)
    }
}

fn copy_error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("String copy emission: {error}"))
}
