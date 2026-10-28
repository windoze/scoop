//! Copy into existing byte storage without retaining interior pointers.

use super::*;
use scoop_lir::ByteCopySource;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_byte_copy(&mut self, instruction: &Instruction) -> Result<(), CodegenError> {
        let Instruction::ByteCopy {
            source,
            target,
            start,
            array_type,
        } = instruction
        else {
            unreachable!("byte copy instruction dispatch is exhaustive");
        };
        let (source, length) = match *source {
            ByteCopySource::String(string) => {
                let length = self.copy_byte_count(string)?;
                let source = self.value(string)?.into_pointer_value();
                (self.byte_gep(source, 24, "bytes.source")?, length)
            }
            ByteCopySource::Prefix {
                array,
                array_type,
                length,
            } => {
                let offset = self
                    .array_type(array_type)
                    .0
                    .layout
                    .instance()
                    .inline_offset();
                let source = self.value(array)?.into_pointer_value();
                (
                    self.byte_gep(source, offset, "bytes.source")?,
                    self.value(length)?.into_int_value(),
                )
            }
            ByteCopySource::Raw { pointer, length } => (
                self.value(pointer)?.into_pointer_value(),
                self.value(length)?.into_int_value(),
            ),
        };
        let offset = self
            .array_type(*array_type)
            .0
            .layout
            .instance()
            .inline_offset();
        let target = self.value(*target)?.into_pointer_value();
        let target = self.byte_gep(target, offset, "bytes.target")?;
        let start = self.value(*start)?.into_int_value();
        // SAFETY: the typed operation carries established, nonoverlapping ranges.
        // No allocation, call, or safepoint separates either address from memcpy.
        let target = unsafe {
            self.builder
                .build_gep(self.context.i8_type(), target, &[start], "bytes.start")
        }
        .map_err(byte_copy_error)?;
        self.builder
            .build_memcpy(target, 1, source, 1, length)
            .map_err(byte_copy_error)?;
        Ok(())
    }
}

fn byte_copy_error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("byte copy emission: {error}"))
}
