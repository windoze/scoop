use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_array_clone(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::ArrayClone {
                source_type,
                out,
                operand,
                array_type,
                safepoint,
                live,
            } => {
                let (array_metadata, target_td) = self.array_type(*array_type);
                let element = array_metadata.element.clone();
                let (source_metadata, source_td) = self.array_type(*source_type);
                if source_metadata.element_exact != array_metadata.element_exact
                    || source_metadata.element != array_metadata.element
                    || source_metadata.layout != array_metadata.layout
                {
                    return Err(CodegenError(
                        "array clone source and target element storage disagree".to_string(),
                    ));
                }
                let operand_ty = function.value_ty(self.globals_arena, *operand);
                if function.temps[*out].ty != scoop_lir::MANAGED_PTR
                    || operand_ty != scoop_lir::MANAGED_PTR
                    || validation::contains_machine_scalar(self.structs, self.enums, &element)
                {
                    return Err(CodegenError(format!(
                        "array_clone @{} requires a machine-scalar-free managed array -> managed array",
                        function.symbol()
                    )));
                }
                let safepoint = self.safepoint_id(*safepoint);
                let live = self.materialize_statepoint_live(live, safepoint)?;
                let operand = self.statepoint_value(*operand, &live)?;
                // The target descriptor is explicit: converting Array<T> to
                // MutableArray<T> (or back) changes nominal runtime identity.
                let clone = self.runtime_fn(
                    scoop_lir::RuntimeAbiSymbolV1::ArrayClone.logical_symbol(),
                    managed_ptr_ty(context, self.managed_address_space).fn_type(
                        &[
                            managed_ptr_ty(context, self.managed_address_space).into(),
                            ptr_ty(context).into(),
                            ptr_ty(context).into(),
                        ],
                        false,
                    ),
                );
                let name = format!("t{}", out.into_raw().into_u32());
                let call = builder
                    .build_call(
                        clone,
                        &[
                            operand.into(),
                            source_td.as_pointer_value().into(),
                            target_td.as_pointer_value().into(),
                        ],
                        &name,
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_clone @{symbol}: {e}",
                            symbol = function.symbol()
                        ))
                    })?;
                self.apply_safepoint_id(call, safepoint);
                let result = call.try_as_basic_value().basic().ok_or_else(|| {
                    CodegenError(format!(
                        "call @{} produced no value",
                        scoop_lir::RuntimeAbiSymbolV1::ArrayClone.logical_symbol()
                    ))
                })?;
                self.restore_statepoint_live(live, safepoint)?;
                self.temps.insert(*out, result);
            }
            _ => unreachable!("array instruction dispatch is exhaustive"),
        }
        Ok(())
    }
}
