use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_aggregate_instruction(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::MakeAggregate { out, elements } => {
                let name = format!("t{}", out.into_raw().into_u32());
                let lir_ty = &function.temps[*out].ty;
                let expected_elements = match lir_ty {
                    LirType::Aggregate(elements) => elements.clone(),
                    LirType::Struct(id) => (0..self.structs[*id].field_count())
                        .map(|index| {
                            self.structs[*id]
                                .field_storage_type(index)
                                .expect("index is below the field count")
                        })
                        .collect::<Vec<_>>(),
                    other => {
                        return Err(CodegenError(format!(
                            "aggregate construction @{} has non-aggregate result type {}",
                            function.symbol(),
                            other.dump()
                        )));
                    }
                };
                if elements.len() != expected_elements.len() {
                    return Err(CodegenError(format!(
                        "aggregate construction @{} has {} elements, expected {}",
                        function.symbol(),
                        elements.len(),
                        expected_elements.len()
                    )));
                }
                for (index, (element, expected)) in
                    elements.iter().zip(&expected_elements).enumerate()
                {
                    let actual = function.value_ty(self.globals_arena, *element);
                    if actual != *expected {
                        return Err(CodegenError(format!(
                            "aggregate construction @{} element {} has type {}, expected {}",
                            function.symbol(),
                            index,
                            actual.dump(),
                            expected.dump()
                        )));
                    }
                }
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    lir_ty,
                )?
                .into_struct_type();
                let aggregate = if let LirType::Struct(id) = lir_ty
                    && self.structs[*id].is_c_layout()
                {
                    let definition = &self.structs[*id];
                    let payload_ty = ty
                        .get_field_type_at_index(1)
                        .expect("C-layout struct has an aligned payload")
                        .into_struct_type();
                    let mut payload = payload_ty.get_undef();
                    for (index, element) in elements.iter().enumerate() {
                        let physical = c_physical_field_index(
                            self.structs,
                            self.enums,
                            definition,
                            index as u32,
                        )?;
                        payload = builder
                            .build_insert_value(payload, self.value(*element)?, physical, &name)
                            .map_err(|e| {
                                CodegenError(format!(
                                    "insert C-layout field @{symbol}: {e}",
                                    symbol = function.symbol()
                                ))
                            })?
                            .into_struct_value();
                    }
                    builder
                        .build_insert_value(ty.get_undef(), payload, 1, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "insert C-layout payload @{symbol}: {e}",
                                symbol = function.symbol()
                            ))
                        })?
                        .into_struct_value()
                } else {
                    let mut aggregate = ty.get_undef();
                    for (index, element) in elements.iter().enumerate() {
                        aggregate = builder
                            .build_insert_value(
                                aggregate,
                                self.value(*element)?,
                                index as u32,
                                &name,
                            )
                            .map_err(|e| {
                                CodegenError(format!(
                                    "insertvalue @{symbol}: {e}",
                                    symbol = function.symbol()
                                ))
                            })?
                            .into_struct_value();
                    }
                    aggregate
                };
                self.temps.insert(*out, aggregate.into());
            }
            Instruction::ExtractValue {
                out,
                aggregate,
                index,
            } => {
                let name = format!("t{}", out.into_raw().into_u32());
                let aggregate_ty = function.value_ty(self.globals_arena, *aggregate);
                let expected = match &aggregate_ty {
                    LirType::Aggregate(elements) => elements.get(*index as usize).cloned(),
                    LirType::Struct(id) => self.structs[*id].field_storage_type(*index as usize),
                    _ => None,
                }
                .ok_or_else(|| {
                    CodegenError(format!(
                        "aggregate extraction @{} has invalid index {} for {}",
                        function.symbol(),
                        index,
                        aggregate_ty.dump()
                    ))
                })?;
                let out_ty = &function.temps[*out].ty;
                if out_ty != &expected {
                    return Err(CodegenError(format!(
                        "aggregate extraction @{} produces {}, expected {}",
                        function.symbol(),
                        out_ty.dump(),
                        expected.dump()
                    )));
                }
                let aggregate = self.value(*aggregate)?.into_struct_value();
                let element = if let LirType::Struct(id) = aggregate_ty
                    && self.structs[id].is_c_layout()
                {
                    let payload = builder
                        .build_extract_value(aggregate, 1, "c_layout_payload")
                        .map_err(|e| {
                            CodegenError(format!(
                                "extract C-layout payload @{symbol}: {e}",
                                symbol = function.symbol()
                            ))
                        })?
                        .into_struct_value();
                    let physical = c_physical_field_index(
                        self.structs,
                        self.enums,
                        &self.structs[id],
                        *index,
                    )?;
                    builder
                        .build_extract_value(payload, physical, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "extract C-layout field @{symbol}: {e}",
                                symbol = function.symbol()
                            ))
                        })?
                } else {
                    builder
                        .build_extract_value(aggregate, *index, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "extractvalue @{symbol}: {e}",
                                symbol = function.symbol()
                            ))
                        })?
                };
                self.temps.insert(*out, element);
            }
            _ => unreachable!("instruction dispatcher routes only aggregate instructions"),
        }
        Ok(())
    }
}
