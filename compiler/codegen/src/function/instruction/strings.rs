//! A fused String operation keeps references, never interior pointers, across GC.

use super::*;
use crate::function::gc::AllocationLayout;
use scoop_lir::StringConcatPart;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_string_concat(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let Instruction::StringConcat {
            out,
            parts,
            descriptor,
            overflow_message,
            safepoint,
            live,
        } = instruction
        else {
            unreachable!("String instruction dispatch is exhaustive");
        };
        let context = self.context;
        let builder = self.builder;
        let word = context.i64_type();
        let mut total = word.const_zero();
        let mut lengths = Vec::with_capacity(parts.len());
        for part in parts {
            let length = match part {
                StringConcatPart::Literal(text) => word.const_int(text.len() as u64, false),
                StringConcatPart::Value(value) => {
                    let object = self.value(*value)?.into_pointer_value();
                    let address = self.byte_gep(object, 16, "concat.length_ptr")?;
                    builder
                        .build_load(word, address, "concat.length")
                        .map_err(string_error)?
                        .into_int_value()
                }
            };
            // Each valid input length fits Long; the checked preceding sum does too.
            let next = builder
                .build_int_add(total, length, "concat.total")
                .map_err(string_error)?;
            let overflow = builder
                .build_int_compare(
                    IntPredicate::UGT,
                    next,
                    word.const_int(i64::MAX as u64 - 31, false),
                    "concat.overflow",
                )
                .map_err(string_error)?;
            self.array_size_check(overflow, *overflow_message, "concat.size.ok")?;
            total = next;
            lengths.push(length);
        }
        let bytes = builder
            .build_int_add(total, word.const_int(24, false), "concat.size")
            .map_err(string_error)?;
        let descriptor = self
            .value(Value::TypeDescriptor(*descriptor))?
            .into_pointer_value();
        let safepoint = self.safepoint_id(*safepoint);
        let live = self.materialize_statepoint_live(live, safepoint)?;
        let result = self.managed_alloc_value(
            descriptor,
            bytes,
            Some(AllocationLayout::string()),
            safepoint,
            live,
            None,
        )?;
        let length_ptr = self.byte_gep(result, 16, "concat.result_length")?;
        builder
            .build_store(length_ptr, total)
            .map_err(string_error)?;
        let data = self.byte_gep(result, 24, "concat.result_bytes")?;
        let mut offset = word.const_zero();
        for (part, length) in parts.iter().zip(lengths) {
            // SAFETY: checked total and allocation bound this whole copy.
            let destination = unsafe {
                builder.build_gep(context.i8_type(), data, &[offset], "concat.destination")
            }
            .map_err(string_error)?;
            match part {
                StringConcatPart::Literal(text) => {
                    if !text.is_empty() {
                        let store = builder
                            .build_store(destination, context.const_string(text.as_bytes(), false))
                            .map_err(string_error)?;
                        store
                            .set_alignment(1)
                            .map_err(|error| CodegenError(error.to_string()))?;
                    }
                }
                StringConcatPart::Value(value) => {
                    // managed_alloc_value restored the canonical, possibly moved, input.
                    let source = self.value(*value)?.into_pointer_value();
                    let source = self.byte_gep(source, 24, "concat.source")?;
                    builder
                        .build_memcpy(destination, 1, source, 1, length)
                        .map_err(string_error)?;
                }
            }
            offset = builder
                .build_int_add(offset, length, "concat.offset")
                .map_err(string_error)?;
        }
        self.temps.insert(*out, result.into());
        Ok(())
    }
}

fn string_error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("String concatenation emission: {error}"))
}
