use super::*;
use scoop_lir::{BoxPayload, NoGcRuntimeFunction, UnboxResult};

mod allocation;
mod unbox;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_boxing(&mut self, instruction: &Instruction) -> Result<(), CodegenError> {
        match instruction {
            Instruction::BoxValue {
                out,
                payload,
                safepoint,
                live,
            } => {
                let safepoint = self.safepoint_id(*safepoint);
                let live = self.materialize_statepoint_live(live, safepoint)?;
                let descriptor = self
                    .value(Value::TypeDescriptor(payload.descriptor()))?
                    .into_pointer_value();
                let result = self.allocate_box(payload, descriptor, safepoint, live)?;
                self.temps.insert(*out, result.into());
            }
            Instruction::UnboxValue { object, result } => {
                self.emit_unbox(*object, result)?;
            }
            _ => unreachable!("boxing instruction dispatch is exhaustive"),
        }
        Ok(())
    }

    fn box_leaf(
        &self,
        runtime: NoGcRuntimeFunction,
        args: &[inkwell::values::BasicMetadataValueEnum<'ctx>],
    ) -> Result<(), CodegenError> {
        let params = args
            .iter()
            .map(|arg| match arg {
                inkwell::values::BasicMetadataValueEnum::PointerValue(value) => {
                    Ok(value.get_type().into())
                }
                inkwell::values::BasicMetadataValueEnum::IntValue(value) => {
                    Ok(value.get_type().into())
                }
                _ => Err(CodegenError(
                    "boxing leaf ABI accepts only pointers and its internal root count".into(),
                )),
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let callee = self.gc_leaf_fn(
            scoop_lir::RuntimeFunction::NoGc(runtime).symbol(),
            self.context.void_type().fn_type(&params, false),
        );
        let call = self
            .builder
            .build_call(callee, args, "")
            .map_err(box_error)?;
        self.apply_nounwind(call);
        Ok(())
    }
}

fn box_error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("box/unbox emission: {error}"))
}
