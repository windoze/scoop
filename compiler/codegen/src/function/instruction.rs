use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// Materialize an operand as an LLVM value: locals are loaded from
    /// their stack slot, temps are SSA values, globals are addressed by
    /// pointer.
    pub(super) fn value(&self, value: Value) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let context = self.context;
        let function = self.function;
        Ok(match value {
            Value::Local(id) => {
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &function.locals[id].ty,
                )?;
                self.builder
                    .build_load(ty, self.allocas[arena_index(id)], &function.locals[id].name)
                    .map_err(|e| CodegenError(format!("load %{}: {e}", function.locals[id].name)))?
            }
            Value::Param(index) => {
                let source = scoop_lir::CallerRootSource::Param(index);
                if let Some(storage) = self.root_storage.get(&source) {
                    self.builder
                        .build_load(storage.ty, storage.pointer, "root_param")
                        .map_err(|error| {
                            CodegenError(format!("load rooted param {index}: {error}"))
                        })?
                } else {
                    self.llvm_function
                        .get_nth_param(index + self.param_offset)
                        .ok_or_else(|| CodegenError(format!("param {index} out of range")))?
                }
            }
            Value::Temp(id) => {
                let source = scoop_lir::CallerRootSource::Temp(id);
                if let Some(storage) = self.root_storage.get(&source) {
                    self.builder
                        .build_load(storage.ty, storage.pointer, "root_temp")
                        .map_err(|error| {
                            CodegenError(format!(
                                "load rooted temp t{}: {error}",
                                id.into_raw().into_u32()
                            ))
                        })?
                } else {
                    *self.temps.get(&id).ok_or_else(|| {
                        CodegenError(format!(
                            "temp t{} used before definition",
                            id.into_raw().into_u32()
                        ))
                    })?
                }
            }
            Value::IntConst(value) => context.i64_type().const_int(value as u64, true).into(),
            Value::BoolConst(value) => context.bool_type().const_int(value as u64, false).into(),
            Value::NullPointer(kind) => pointer_ty(context, self.managed_address_space, kind)
                .const_null()
                .into(),
            Value::TypeDescriptor(reference) => {
                type_descriptor_global(reference, self.type_tds, self.external_type_tds)?
                    .as_pointer_value()
                    .into()
            }
            Value::RootScan(id) => self.root_scans[arena_index(id)].into(),
            Value::Global(id) => self.globals[arena_index(id)]
                .expect("ordinary globals are emitted")
                .as_pointer_value()
                .into(),
        })
    }

    pub(super) fn statepoint_value(
        &self,
        value: Value,
        live: &MaterializedStatepointLive<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let source = match value {
            Value::Param(index) => Some(scoop_lir::CallerRootSource::Param(index)),
            Value::Local(id) => Some(scoop_lir::CallerRootSource::Local(id)),
            Value::Temp(id) => Some(scoop_lir::CallerRootSource::Temp(id)),
            Value::IntConst(_)
            | Value::BoolConst(_)
            | Value::NullPointer(_)
            | Value::TypeDescriptor(_)
            | Value::RootScan(_)
            | Value::Global(_) => None,
        };
        source
            .and_then(|source| live.arguments.get(&source).copied())
            .map_or_else(|| self.value(value), Ok)
    }

    pub(super) fn sync_root_temp(&self, temp: TempId) -> Result<(), CodegenError> {
        let source = scoop_lir::CallerRootSource::Temp(temp);
        let Some(storage) = self.root_storage.get(&source) else {
            return Ok(());
        };
        let value = self.temps.get(&temp).ok_or_else(|| {
            CodegenError(format!(
                "rooted temp t{} has no emitted definition",
                temp.into_raw().into_u32()
            ))
        })?;
        self.builder
            .build_store(storage.pointer, *value)
            .map_err(|error| {
                CodegenError(format!(
                    "store rooted temp t{}: {error}",
                    temp.into_raw().into_u32()
                ))
            })?;
        Ok(())
    }

    pub(super) fn instruction(&mut self, instruction: &Instruction) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::BinOp { out, op, lhs, rhs } => {
                let lhs = self.value(*lhs)?;
                let rhs = self.value(*rhs)?;
                let name = format!("t{}", out.into_raw().into_u32());
                let result: IntValue = match (lhs, rhs) {
                    (BasicValueEnum::PointerValue(lhs), BasicValueEnum::PointerValue(rhs)) => {
                        let predicate = match op {
                            BinOp::Eq => IntPredicate::EQ,
                            BinOp::Ne => IntPredicate::NE,
                            _ => {
                                return Err(CodegenError(format!(
                                    "pointer operands only support equality in @{}",
                                    function.symbol
                                )));
                            }
                        };
                        builder.build_int_compare(predicate, lhs, rhs, &name)
                    }
                    (BasicValueEnum::PointerValue(_), _) | (_, BasicValueEnum::PointerValue(_)) => {
                        return Err(CodegenError(format!(
                            "binary operands mix pointer and value types in @{}",
                            function.symbol
                        )));
                    }
                    (lhs, rhs) => {
                        let lhs = lhs.into_int_value();
                        let rhs = rhs.into_int_value();
                        match op {
                            BinOp::Add => builder.build_int_add(lhs, rhs, &name),
                            BinOp::Sub => builder.build_int_sub(lhs, rhs, &name),
                            BinOp::Mul => builder.build_int_mul(lhs, rhs, &name),
                            BinOp::SDiv => builder.build_int_signed_div(lhs, rhs, &name),
                            BinOp::Lt => {
                                builder.build_int_compare(IntPredicate::SLT, lhs, rhs, &name)
                            }
                            BinOp::Le => {
                                builder.build_int_compare(IntPredicate::SLE, lhs, rhs, &name)
                            }
                            BinOp::Gt => {
                                builder.build_int_compare(IntPredicate::SGT, lhs, rhs, &name)
                            }
                            BinOp::Ge => {
                                builder.build_int_compare(IntPredicate::SGE, lhs, rhs, &name)
                            }
                            BinOp::Eq => {
                                builder.build_int_compare(IntPredicate::EQ, lhs, rhs, &name)
                            }
                            BinOp::Ne => {
                                builder.build_int_compare(IntPredicate::NE, lhs, rhs, &name)
                            }
                        }
                    }
                }
                .map_err(|e| {
                    CodegenError(format!("{op:?} @{symbol}: {e}", symbol = function.symbol))
                })?;
                self.temps.insert(*out, result.into());
            }
            Instruction::UnaryOp { out, op, operand } => {
                let operand = self.value(*operand)?.into_int_value();
                let name = format!("t{}", out.into_raw().into_u32());
                let result = match op {
                    // Neg is `0 - x` (no dedicated LLVM neg instruction).
                    UnOp::Neg => {
                        builder.build_int_sub(operand.get_type().const_zero(), operand, &name)
                    }
                    // Not is `xor x, true`: flips the i1 bit directly.
                    UnOp::Not => {
                        builder.build_xor(operand, operand.get_type().const_all_ones(), &name)
                    }
                }
                .map_err(|e| {
                    CodegenError(format!("{op:?} @{symbol}: {e}", symbol = function.symbol))
                })?;
                self.temps.insert(*out, result.into());
            }
            Instruction::MakeAggregate { out, elements } => {
                let name = format!("t{}", out.into_raw().into_u32());
                let lir_ty = &function.temps[*out].ty;
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    lir_ty,
                )?
                .into_struct_type();
                let aggregate = if let LirType::Struct(id) = lir_ty
                    && self.structs[*id].c_layout.is_some()
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
                                    symbol = function.symbol
                                ))
                            })?
                            .into_struct_value();
                    }
                    builder
                        .build_insert_value(ty.get_undef(), payload, 1, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "insert C-layout payload @{symbol}: {e}",
                                symbol = function.symbol
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
                                    symbol = function.symbol
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
                let aggregate = self.value(*aggregate)?.into_struct_value();
                let element = if let LirType::Struct(id) = aggregate_ty
                    && self.structs[id].c_layout.is_some()
                {
                    let payload = builder
                        .build_extract_value(aggregate, 1, "c_layout_payload")
                        .map_err(|e| {
                            CodegenError(format!(
                                "extract C-layout payload @{symbol}: {e}",
                                symbol = function.symbol
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
                                symbol = function.symbol
                            ))
                        })?
                } else {
                    builder
                        .build_extract_value(aggregate, *index, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "extractvalue @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?
                };
                self.temps.insert(*out, element);
            }
            Instruction::HeapLoad {
                out,
                object,
                offset,
            } => {
                let name = format!("t{}", out.into_raw().into_u32());
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "field_ptr")?;
                let field_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &function.temps[*out].ty,
                )?;
                let element = builder
                    .build_load(field_ty, field_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "heap load @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, element);
            }
            Instruction::AtomicLoad {
                out,
                object,
                offset,
            } => {
                if *offset < 16 {
                    return Err(CodegenError(format!(
                        "atomic_load @{symbol}: offset {offset} is inside the object header",
                        symbol = function.symbol,
                        offset = *offset
                    )));
                }
                let name = format!("t{}", out.into_raw().into_u32());
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "atomic_field_ptr")?;
                let field_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &function.temps[*out].ty,
                )?;
                let element = builder
                    .build_load(field_ty, field_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic load @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                element
                    .as_instruction_value()
                    .expect("a load is an instruction")
                    .set_atomic_ordering(AtomicOrdering::Acquire)
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic load ordering @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, element);
            }
            Instruction::HeapStore {
                object,
                offset,
                value,
            } => {
                // Object fields begin after the 16-byte header.
                if *offset < 16 {
                    return Err(CodegenError(format!(
                        "heap_store @{symbol}: offset {offset} is inside the object header",
                        symbol = function.symbol,
                        offset = *offset
                    )));
                }
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "field_ptr")?;
                builder
                    .build_store(field_ptr, self.value(*value)?)
                    .map_err(|e| {
                        CodegenError(format!(
                            "heap_store @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                // M9 write barrier: mark the stored-to address's card.
                self.card_mark(field_ptr)?;
            }
            Instruction::AtomicStore {
                object,
                offset,
                value,
            } => {
                if *offset < 16 {
                    return Err(CodegenError(format!(
                        "atomic_store @{symbol}: offset {offset} is inside the object header",
                        symbol = function.symbol,
                        offset = *offset
                    )));
                }
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "atomic_field_ptr")?;
                let store = builder
                    .build_store(field_ptr, self.value(*value)?)
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic store @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                store
                    .set_atomic_ordering(AtomicOrdering::Release)
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic store ordering @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
            }
            Instruction::AtomicCompareExchange {
                out,
                object,
                offset,
                expected,
                replacement,
            } => {
                if *offset < 16 {
                    return Err(CodegenError(format!(
                        "atomic_cmpxchg @{symbol}: offset {offset} is inside the object header",
                        symbol = function.symbol,
                        offset = *offset
                    )));
                }
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "atomic_field_ptr")?;
                let pair = builder
                    .build_cmpxchg(
                        field_ptr,
                        self.value(*expected)?.into_int_value(),
                        self.value(*replacement)?.into_int_value(),
                        AtomicOrdering::AcquireRelease,
                        AtomicOrdering::Acquire,
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic cmpxchg @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                let old = builder
                    .build_extract_value(pair, 0, &format!("t{}", out.into_raw().into_u32()))
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic cmpxchg result @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, old);
            }
            Instruction::FunctionAddress { out, symbol } => {
                let function_value = self.llvm.get_function(symbol).ok_or_else(|| {
                    CodegenError(format!(
                        "function_address @{}: unknown function @{}",
                        function.symbol, symbol
                    ))
                })?;
                self.temps.insert(
                    *out,
                    function_value.as_global_value().as_pointer_value().into(),
                );
            }
            Instruction::ForeignCallbackRegister {
                out,
                bridge,
                closure,
            } => {
                let bridge = &self.foreign_callback_bridges[*bridge];
                let closure = self.value(*closure)?.into_pointer_value();
                let adapter = self
                    .llvm
                    .get_function(&bridge.adapter_symbol)
                    .ok_or_else(|| {
                        CodegenError(format!(
                            "foreign callback adapter @{} was not emitted",
                            bridge.adapter_symbol
                        ))
                    })?
                    .as_global_value()
                    .as_pointer_value();
                let signature = self
                    .llvm
                    .get_global(&bridge.signature_symbol)
                    .expect("foreign callback signature descriptor is declared")
                    .as_pointer_value();
                let register = self.gc_leaf_fn(
                    "scoop_runtime_callback_register",
                    ptr_ty(context).fn_type(
                        &[
                            managed_ptr_ty(context, self.managed_address_space).into(),
                            ptr_ty(context).into(),
                            ptr_ty(context).into(),
                            context.i32_type().into(),
                        ],
                        false,
                    ),
                );
                let mode = match bridge.mode {
                    scoop_lir::ForeignCallbackMode::Reusable => 0,
                    scoop_lir::ForeignCallbackMode::OneShot => 1,
                };
                let callback_context = builder
                    .build_call(
                        register,
                        &[
                            closure.into(),
                            adapter.into(),
                            signature.into(),
                            context.i32_type().const_int(mode, false).into(),
                        ],
                        "foreign_callback_context",
                    )
                    .map_err(|error| {
                        CodegenError(format!("foreign callback registration: {error}"))
                    })?
                    .try_as_basic_value()
                    .basic()
                    .expect("callback registration returns a context")
                    .into_pointer_value();
                let trampoline = self
                    .llvm
                    .get_function(&bridge.trampoline_symbol)
                    .expect("foreign callback trampoline is declared")
                    .as_global_value()
                    .as_pointer_value();
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &function.temps[*out].ty,
                )?
                .into_struct_type();
                let value = builder
                    .build_insert_value(ty.get_undef(), trampoline, 0, "callback_function")
                    .and_then(|value| {
                        builder.build_insert_value(
                            value.into_struct_value(),
                            callback_context,
                            1,
                            "callback_value",
                        )
                    })
                    .map_err(|error| {
                        CodegenError(format!("construct foreign callback value: {error}"))
                    })?;
                self.temps.insert(*out, value.into_struct_value().into());
            }
            Instruction::ForeignCallbackOperation(operation) => {
                let callback = self.value(operation.callback())?.into_struct_value();
                let function_pointer = builder
                    .build_extract_value(callback, 0, "callback_function")
                    .map_err(|error| CodegenError(format!("extract callback function: {error}")))?
                    .into_pointer_value();
                let callback_context = builder
                    .build_extract_value(callback, 1, "callback_context")
                    .map_err(|error| CodegenError(format!("extract callback context: {error}")))?
                    .into_pointer_value();
                match *operation {
                    scoop_lir::ForeignCallbackOperation::Retain { out, .. } => {
                        let retain = self.gc_leaf_fn(
                            "scoop_runtime_callback_retain",
                            ptr_ty(context).fn_type(&[ptr_ty(context).into()], false),
                        );
                        let retained = builder
                            .build_call(retain, &[callback_context.into()], "retained_context")
                            .map_err(|error| {
                                CodegenError(format!("retain foreign callback: {error}"))
                            })?
                            .try_as_basic_value()
                            .basic()
                            .expect("retain returns a context")
                            .into_pointer_value();
                        let ty = basic_ty(
                            context,
                            self.structs,
                            self.enums,
                            self.managed_address_space,
                            &function.temps[out].ty,
                        )?
                        .into_struct_type();
                        let value = builder
                            .build_insert_value(
                                ty.get_undef(),
                                function_pointer,
                                0,
                                "callback_function",
                            )
                            .and_then(|value| {
                                builder.build_insert_value(
                                    value.into_struct_value(),
                                    retained,
                                    1,
                                    "retained_callback",
                                )
                            })
                            .map_err(|error| {
                                CodegenError(format!("construct retained callback: {error}"))
                            })?;
                        self.temps.insert(out, value.into_struct_value().into());
                    }
                    scoop_lir::ForeignCallbackOperation::Release { .. } => {
                        let release = self.gc_leaf_fn(
                            "scoop_runtime_callback_release",
                            context
                                .void_type()
                                .fn_type(&[ptr_ty(context).into()], false),
                        );
                        builder
                            .build_call(release, &[callback_context.into()], "")
                            .map_err(|error| {
                                CodegenError(format!("release foreign callback: {error}"))
                            })?;
                    }
                    scoop_lir::ForeignCallbackOperation::Failure { out, .. } => {
                        let failure = self.gc_leaf_fn(
                            "scoop_runtime_callback_failure",
                            managed_ptr_ty(context, self.managed_address_space)
                                .fn_type(&[ptr_ty(context).into()], false),
                        );
                        let value = builder
                            .build_call(failure, &[callback_context.into()], "callback_failure")
                            .map_err(|error| {
                                CodegenError(format!("query callback failure: {error}"))
                            })?
                            .try_as_basic_value()
                            .basic()
                            .expect("failure query returns a nullable managed reference");
                        self.temps.insert(out, value);
                    }
                    scoop_lir::ForeignCallbackOperation::State { out, .. } => {
                        let state = self.gc_leaf_fn(
                            "scoop_runtime_callback_state",
                            context.i32_type().fn_type(&[ptr_ty(context).into()], false),
                        );
                        let state = builder
                            .build_call(state, &[callback_context.into()], "callback_state")
                            .map_err(|error| {
                                CodegenError(format!("query callback state: {error}"))
                            })?
                            .try_as_basic_value()
                            .basic()
                            .expect("state query returns a tag")
                            .into_int_value();
                        let state = builder
                            .build_int_z_extend(state, context.i64_type(), "callback_state_tag")
                            .map_err(|error| {
                                CodegenError(format!("extend callback state: {error}"))
                            })?;
                        let LirType::Enum(enum_id) = function.temps[out].ty else {
                            return Err(CodegenError(
                                "callback state result is not an enum".to_string(),
                            ));
                        };
                        let EnumRepr::Tagged { size, align, .. } = &self.enums[enum_id].repr else {
                            return Err(CodegenError(
                                "callback state enum unexpectedly uses a niche".to_string(),
                            ));
                        };
                        let ty = tagged_ty(
                            context,
                            self.managed_address_space,
                            *size,
                            *align,
                            &self.enums[enum_id].scan,
                        )?;
                        let slot = self.entry_alloca(ty.into(), "callback_state_value")?;
                        builder
                            .build_store(slot, ty.const_zero())
                            .map_err(|error| {
                                CodegenError(format!("zero callback state: {error}"))
                            })?;
                        let tag = self.tag_ptr(slot, ty)?;
                        builder.build_store(tag, state).map_err(|error| {
                            CodegenError(format!("construct callback state: {error}"))
                        })?;
                        let value = builder
                            .build_load(ty, slot, "callback_state_value")
                            .map_err(|error| {
                                CodegenError(format!("load callback state: {error}"))
                            })?;
                        self.temps.insert(out, value);
                    }
                }
            }
            Instruction::IntToPtr { out, value } => {
                let value = self.value(*value)?.into_int_value();
                let result = builder
                    .build_int_to_ptr(value, ptr_ty(context), "raw_ptr")
                    .map_err(|e| CodegenError(format!("inttoptr @{}: {e}", function.symbol)))?;
                self.temps.insert(*out, result.into());
            }
            Instruction::PtrToInt { out, value } => {
                let value = self.value(*value)?.into_pointer_value();
                if value.get_type().get_address_space() == self.managed_address_space.inkwell() {
                    return Err(CodegenError(format!(
                        "managed pointer cannot be lowered by PtrToInt in @{}",
                        function.symbol
                    )));
                }
                let result = builder
                    .build_ptr_to_int(value, context.i64_type(), "raw_uint")
                    .map_err(|e| CodegenError(format!("ptrtoint @{}: {e}", function.symbol)))?;
                self.temps.insert(*out, result.into());
            }
            Instruction::RawLoad {
                out,
                pointer,
                align,
            } => {
                let pointer = self.value(*pointer)?.into_pointer_value();
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &function.temps[*out].ty,
                )?;
                let value = builder
                    .build_load(ty, pointer, "raw_load")
                    .map_err(|e| CodegenError(format!("raw load @{}: {e}", function.symbol)))?;
                value
                    .as_instruction_value()
                    .expect("a non-constant load is an instruction")
                    .set_alignment(*align as u32)
                    .map_err(|e| {
                        CodegenError(format!("raw load alignment @{}: {e}", function.symbol))
                    })?;
                self.temps.insert(*out, value);
            }
            Instruction::RawStore {
                pointer,
                value,
                align,
            } => {
                let pointer = self.value(*pointer)?.into_pointer_value();
                let store = builder
                    .build_store(pointer, self.value(*value)?)
                    .map_err(|e| CodegenError(format!("raw store @{}: {e}", function.symbol)))?;
                store.set_alignment(*align as u32).map_err(|e| {
                    CodegenError(format!("raw store alignment @{}: {e}", function.symbol))
                })?;
            }
            Instruction::PtrOffset {
                out,
                pointer,
                bytes,
            } => {
                let pointer = self.value(*pointer)?.into_pointer_value();
                let bytes = self.value(*bytes)?.into_int_value();
                // SAFETY: source semantics make raw pointer arithmetic unsafe;
                // validity of the resulting address remains the caller's duty.
                let result = unsafe {
                    builder.build_gep(context.i8_type(), pointer, &[bytes], "raw_offset")
                }
                .map_err(|e| CodegenError(format!("raw gep @{}: {e}", function.symbol)))?;
                self.temps.insert(*out, result.into());
            }
            Instruction::LocalAddress { out, local } => {
                self.temps
                    .insert(*out, self.allocas[arena_index(*local)].into());
            }
            Instruction::GlobalLoad { out, global } => {
                let llvm_global =
                    self.globals[arena_index(*global)].expect("storage globals are emitted");
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &function.temps[*out].ty,
                )?;
                let value = builder
                    .build_load(ty, llvm_global.as_pointer_value(), "global_load")
                    .map_err(|error| CodegenError(format!("global load: {error}")))?;
                self.temps.insert(*out, value);
            }
            Instruction::GlobalStore { global, value } => {
                let llvm_global =
                    self.globals[arena_index(*global)].expect("storage globals are emitted");
                builder
                    .build_store(llvm_global.as_pointer_value(), self.value(*value)?)
                    .map_err(|error| CodegenError(format!("global store: {error}")))?;
            }
            Instruction::GlobalAddress { out, global } => {
                let llvm_global =
                    self.globals[arena_index(*global)].expect("storage globals are emitted");
                self.temps
                    .insert(*out, llvm_global.as_pointer_value().into());
            }
            Instruction::NativeGlobalLoad {
                out,
                global,
                safepoint,
                roots,
            } => {
                let native = &self.native_globals[*global];
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &native.ty,
                )?;
                let slot = self.entry_alloca(ty, "native_global_result")?;
                let symbol = &self.native_global_bridges.gets[native.access.get()].symbol;
                let callee = self.native_global_bridge(symbol);
                let transition = self.publish_native_roots(
                    roots.as_slice(),
                    None,
                    NativeTransitionKind::Safe,
                    *safepoint,
                )?;
                self.emit_native_global_call(callee, slot)?;
                self.finish_native_transition(transition, NativeTransitionKind::Safe)?;
                let value = builder
                    .build_load(ty, slot, "native_global_value")
                    .map_err(|error| CodegenError(format!("native global load: {error}")))?;
                self.temps.insert(*out, value);
            }
            Instruction::NativeGlobalStore {
                global,
                value,
                safepoint,
                roots,
            } => {
                let native = &self.native_globals[*global];
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &native.ty,
                )?;
                let slot = self.entry_alloca(ty, "native_global_argument")?;
                builder
                    .build_store(slot, self.value(*value)?)
                    .map_err(|error| CodegenError(format!("native global spill: {error}")))?;
                let scoop_lir::NativeGlobalAccess::Mutable { set, .. } = native.access else {
                    return Err(CodegenError(format!(
                        "native global store targets readonly `{}`",
                        native.source_name
                    )));
                };
                let symbol = &self.native_global_bridges.sets[set].symbol;
                let callee = self.native_global_bridge(symbol);
                let transition = self.publish_native_roots(
                    roots.as_slice(),
                    None,
                    NativeTransitionKind::Safe,
                    *safepoint,
                )?;
                self.emit_native_global_call(callee, slot)?;
                self.finish_native_transition(transition, NativeTransitionKind::Safe)?;
            }
            Instruction::NativeGlobalAddress {
                out,
                global,
                safepoint,
                roots,
            } => {
                let native = &self.native_globals[*global];
                let ty: BasicTypeEnum = ptr_ty(context).into();
                let slot = self.entry_alloca(ty, "native_global_address")?;
                let symbol = &self.native_global_bridges.addresses[native.access.address()].symbol;
                let callee = self.native_global_bridge(symbol);
                let transition = self.publish_native_roots(
                    roots.as_slice(),
                    None,
                    NativeTransitionKind::Safe,
                    *safepoint,
                )?;
                self.emit_native_global_call(callee, slot)?;
                self.finish_native_transition(transition, NativeTransitionKind::Safe)?;
                let value = builder
                    .build_load(ty, slot, "native_global_pointer")
                    .map_err(|error| CodegenError(format!("native global pointer: {error}")))?;
                self.temps.insert(*out, value);
            }
            Instruction::Store { local, value: v } => {
                let operand = self.value(*v)?;
                builder
                    .build_store(self.allocas[arena_index(*local)], operand)
                    .map_err(|e| {
                        CodegenError(format!("store %{}: {e}", function.locals[*local].name))
                    })?;
            }
            Instruction::Call { site } => {
                self.emit_call_site(site)?;
            }
            Instruction::ManagedPoll { site } => {
                self.safepoint_poll(site)?;
            }
            Instruction::Invoke { site } => {
                self.emit_invoke_site(site)?;
            }
            Instruction::LandingPad { record, raw } => {
                // Catch-all landing pad (M8, runtime spec 5). Keep the
                // `{ ptr, i32 }` record and its raw exception pointer
                // separate from BeginCatch: cleanup chains can forward
                // the same exception to an enclosing dispatch block.
                let personality =
                    self.llvm_function
                        .get_personality_function()
                        .ok_or_else(|| {
                            CodegenError(format!(
                                "landingpad @{symbol}: function has no personality function",
                                symbol = function.symbol
                            ))
                        })?;
                let exception_ty = context
                    .struct_type(&[ptr_ty(context).into(), context.i32_type().into()], false);
                let catch_all: BasicValueEnum = ptr_ty(context).const_null().into();
                let landing_pad = builder
                    .build_landing_pad(
                        exception_ty,
                        personality,
                        &[catch_all],
                        false,
                        &format!("t{}", record.into_raw().into_u32()),
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "landingpad @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                let exception_ptr = builder
                    .build_extract_value(landing_pad.into_struct_value(), 0, "exception_ptr")
                    .map_err(|e| {
                        CodegenError(format!(
                            "landingpad @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*record, landing_pad);
                self.temps.insert(*raw, exception_ptr);
                self.finish_unwind_compiler_roots()?;
            }
            Instruction::CleanupPad { record, raw } => {
                // Cleanup-only landing pad for leaving an active catch
                // because of a new exception or a rethrow. It does not
                // call begin_catch; lir-lower either forwards the
                // captured record through ordinary continuation blocks
                // or resumes it after EndCatch balances the handler.
                let personality =
                    self.llvm_function
                        .get_personality_function()
                        .ok_or_else(|| {
                            CodegenError(format!(
                                "cleanup pad @{symbol}: function has no personality function",
                                symbol = function.symbol
                            ))
                        })?;
                let exception_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &LirType::ExceptionRecord,
                )?
                .into_struct_type();
                let landing_pad = builder
                    .build_landing_pad(
                        exception_ty,
                        personality,
                        &[],
                        true,
                        &format!("t{}", record.into_raw().into_u32()),
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "cleanup pad @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                let exception_ptr = builder
                    .build_extract_value(landing_pad.into_struct_value(), 0, "exception_ptr")
                    .map_err(|e| {
                        CodegenError(format!(
                            "cleanup pad @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*record, landing_pad);
                self.temps.insert(*raw, exception_ptr);
                self.finish_unwind_compiler_roots()?;
            }
            Instruction::BeginCatch { out, raw } => {
                let begin_catch = self.gc_leaf_fn(
                    "__cxa_begin_catch",
                    managed_ptr_ty(context, self.managed_address_space)
                        .fn_type(&[ptr_ty(context).into()], false),
                );
                let name = format!("t{}", out.into_raw().into_u32());
                let object = builder
                    .build_call(begin_catch, &[self.value(*raw)?.into()], &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "begin_catch @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?
                    .try_as_basic_value()
                    .basic()
                    .ok_or_else(|| CodegenError("__cxa_begin_catch returned void".to_string()))?;
                self.temps.insert(*out, object);
            }
            Instruction::EndCatch => {
                let end_catch =
                    self.gc_leaf_fn("__cxa_end_catch", context.void_type().fn_type(&[], false));
                builder
                    .build_call(end_catch, &[], "")
                    .map_err(|e| CodegenError(format!("end_catch @{}: {e}", function.symbol)))?;
            }
            Instruction::Throw { exception } => {
                // `void scoop_rt_throw(ptr)` (noreturn; runtime spec 5).
                // The block's Unreachable terminator emits the LLVM
                // `unreachable` after the call, like the trap path.
                let throw = self.gc_leaf_fn(
                    "scoop_rt_throw",
                    context.void_type().fn_type(
                        &[managed_ptr_ty(context, self.managed_address_space).into()],
                        false,
                    ),
                );
                throw.add_attribute(
                    AttributeLoc::Function,
                    context.create_enum_attribute(Attribute::get_named_enum_kind_id("noreturn"), 0),
                );
                builder
                    .build_call(throw, &[self.value(*exception)?.into()], "")
                    .map_err(|e| {
                        CodegenError(format!("throw @{symbol}: {e}", symbol = function.symbol))
                    })?;
            }
            Instruction::ArrayAlloc {
                out,
                elements,
                array_type,
                safepoint,
                live,
            } => {
                let live = self.materialize_statepoint_live(live, *safepoint)?;
                // `{ ptr td, i64 gc_word, i64 size, [n x elem] }`
                // (runtime spec 2.5; the 16-byte header is M9):
                // allocate align_up(24, element_align) + n * stride
                // bytes, store the size at offset 16, then store each
                // element in order. The rounded data offset is visible
                // for over-aligned C-layout elements.
                let (array_metadata, td) = self.array_type(*array_type);
                let element_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &array_metadata.element,
                )?;
                let stride = array_metadata.element_size;
                let data_offset = array_data_offset(array_metadata.element_align);
                let td = td.as_pointer_value();
                let total = data_offset + elements.len() as u64 * stride;
                let array = self.managed_alloc_value(
                    td,
                    context.i64_type().const_int(total, false),
                    *safepoint,
                    live,
                )?;
                let size_ptr = self.byte_gep(array, 16, "size_ptr")?;
                builder
                    .build_store(
                        size_ptr,
                        context.i64_type().const_int(elements.len() as u64, false),
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "array size @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                for (index, element_value) in elements.iter().enumerate() {
                    let element_ptr = self.element_ptr(
                        array,
                        element_ty,
                        context.i64_type().const_int(index as u64, false),
                        "element_ptr",
                    )?;
                    builder
                        .build_store(element_ptr, self.value(*element_value)?)
                        .map_err(|e| {
                            CodegenError(format!(
                                "array element @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?;
                }
                self.temps.insert(*out, array.into());
            }
            Instruction::ArrayLen { out, operand, .. } => {
                let array = self.value(*operand)?.into_pointer_value();
                let size_ptr = self.byte_gep(array, 16, "size_ptr")?;
                let name = format!("t{}", out.into_raw().into_u32());
                let size = builder
                    .build_load(context.i64_type(), size_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_len @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, size);
            }
            Instruction::ArrayGet {
                out,
                array,
                index,
                array_type,
            } => {
                let (array_metadata, _) = self.array_type(*array_type);
                let element_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &array_metadata.element,
                )?;
                let array = self.value(*array)?.into_pointer_value();
                let index = self.value(*index)?.into_int_value();
                self.bounds_check(array, index)?;
                let element_ptr = self.element_ptr(array, element_ty, index, "element_ptr")?;
                let name = format!("t{}", out.into_raw().into_u32());
                let value = builder
                    .build_load(element_ty, element_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_get @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, value);
            }
            Instruction::ArraySet {
                array,
                index,
                value,
                array_type,
            } => {
                let (array_metadata, _) = self.array_type(*array_type);
                let element_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &array_metadata.element,
                )?;
                let array = self.value(*array)?.into_pointer_value();
                let index = self.value(*index)?.into_int_value();
                self.bounds_check(array, index)?;
                let element_ptr = self.element_ptr(array, element_ty, index, "element_ptr")?;
                builder
                    .build_store(element_ptr, self.value(*value)?)
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_set @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                // M9 write barrier: mark the stored-to address's card
                // (array element stores are heap stores too).
                self.card_mark(element_ptr)?;
            }
            Instruction::ArrayClone {
                out,
                operand,
                array_type,
                safepoint,
                live,
            } => {
                let live = self.materialize_statepoint_live(live, *safepoint)?;
                let operand = self.statepoint_value(*operand, &live)?;
                // The target descriptor is explicit: converting Array<T> to
                // MutableArray<T> (or back) changes nominal runtime identity.
                let (array_metadata, target_td) = self.array_type(*array_type);
                let stride = array_metadata.element_size;
                let data_offset = array_data_offset(array_metadata.element_align);
                let clone = self.runtime_fn(
                    scoop_lir::ARRAY_CLONE_SYMBOL,
                    managed_ptr_ty(context, self.managed_address_space).fn_type(
                        &[
                            managed_ptr_ty(context, self.managed_address_space).into(),
                            ptr_ty(context).into(),
                            context.i64_type().into(),
                            context.i64_type().into(),
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
                            target_td.as_pointer_value().into(),
                            context.i64_type().const_int(stride, false).into(),
                            context.i64_type().const_int(data_offset, false).into(),
                        ],
                        &name,
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_clone @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.apply_safepoint_id(call, *safepoint);
                let result = call.try_as_basic_value().basic().ok_or_else(|| {
                    CodegenError(format!(
                        "call @{} produced no value",
                        scoop_lir::ARRAY_CLONE_SYMBOL
                    ))
                })?;
                self.restore_statepoint_live(live, *safepoint)?;
                self.temps.insert(*out, result);
            }
            Instruction::EnumWrap {
                out,
                enum_id,
                variant,
                fields,
            } => {
                let def = &self.enums[*enum_id];
                let name = format!("t{}", out.into_raw().into_u32());
                let result: BasicValueEnum = match &def.repr {
                    EnumRepr::Niche { payload_variant } => {
                        if *variant == *payload_variant {
                            // The payload is the bare pointer itself.
                            self.value(fields[0])?
                        } else {
                            // The payload-less variant is the null pointer.
                            basic_ty(
                                context,
                                self.structs,
                                self.enums,
                                self.managed_address_space,
                                &function.temps[*out].ty,
                            )?
                            .into_pointer_type()
                            .const_null()
                            .into()
                        }
                    }
                    EnumRepr::Tagged {
                        variants,
                        size,
                        align,
                    } => {
                        // The full value is zero before tag/payload writes,
                        // so every inactive ref-bearing slot is safe for
                        // unconditional GC scanning.
                        let ty = tagged_ty(
                            context,
                            self.managed_address_space,
                            *size,
                            *align,
                            &def.scan,
                        )?;
                        let slot = self.entry_alloca(ty.into(), "enum_wrap")?;
                        builder.build_store(slot, ty.const_zero()).map_err(|e| {
                            CodegenError(format!(
                                "enum_wrap zero @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?;
                        let tag_ptr = self.tag_ptr(slot, ty)?;
                        builder
                            .build_store(
                                tag_ptr,
                                context.i64_type().const_int(*variant as u64, false),
                            )
                            .map_err(|e| {
                                CodegenError(format!(
                                    "enum_wrap tag @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?;
                        let variant_repr = &variants[*variant as usize];
                        for (value, field) in fields.iter().zip(&variant_repr.fields) {
                            let field_ptr = self.enum_field_ptr(slot, field.offset, "field_ptr")?;
                            builder
                                .build_store(field_ptr, self.value(*value)?)
                                .map_err(|e| {
                                    CodegenError(format!(
                                        "enum_wrap field @{symbol}: {e}",
                                        symbol = function.symbol
                                    ))
                                })?;
                        }
                        builder.build_load(ty, slot, &name).map_err(|e| {
                            CodegenError(format!(
                                "enum_wrap @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?
                    }
                };
                self.temps.insert(*out, result);
            }
            Instruction::EnumTag {
                out,
                enum_id,
                operand,
            } => {
                let def = &self.enums[*enum_id];
                let operand = self.value(*operand)?;
                let name = format!("t{}", out.into_raw().into_u32());
                let result: BasicValueEnum = match &def.repr {
                    // Niche: the tag value is the variant index — null ↔
                    // the payload-less variant, non-null ↔ the payload
                    // variant.
                    EnumRepr::Niche { payload_variant } => {
                        let operand = operand.into_pointer_value();
                        let non_null = builder
                            .build_int_compare(
                                IntPredicate::NE,
                                operand,
                                operand.get_type().const_null(),
                                "non_null",
                            )
                            .map_err(|e| {
                                CodegenError(format!(
                                    "enum_tag @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?;
                        let i64_ty = context.i64_type();
                        builder
                            .build_select(
                                non_null,
                                i64_ty.const_int(*payload_variant as u64, false),
                                i64_ty.const_int((1 - *payload_variant) as u64, false),
                                &name,
                            )
                            .map_err(|e| {
                                CodegenError(format!(
                                    "enum_tag @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?
                    }
                    EnumRepr::Tagged { .. } => builder
                        .build_extract_value(operand.into_struct_value(), 0, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "enum_tag @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?,
                };
                self.temps.insert(*out, result);
            }
            Instruction::EnumField {
                out,
                enum_id,
                variant,
                index,
                operand,
            } => {
                let def = &self.enums[*enum_id];
                let operand = self.value(*operand)?;
                let name = format!("t{}", out.into_raw().into_u32());
                let result: BasicValueEnum = match &def.repr {
                    // Niche: the payload is the pointer itself; lir-lower
                    // only emits this on paths where the tag is known.
                    EnumRepr::Niche { .. } => operand,
                    EnumRepr::Tagged {
                        variants,
                        size,
                        align,
                    } => {
                        // Reverse of EnumWrap: spill the aggregate into an
                        // entry-block alloca, then load the field out of
                        // the payload area.
                        let ty = tagged_ty(
                            context,
                            self.managed_address_space,
                            *size,
                            *align,
                            &def.scan,
                        )?;
                        let slot = self.entry_alloca(ty.into(), "enum_field")?;
                        builder.build_store(slot, operand).map_err(|e| {
                            CodegenError(format!(
                                "enum_field @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?;
                        let variant_repr = &variants[*variant as usize];
                        let field = &variant_repr.fields[*index as usize];
                        let field_ptr = self.enum_field_ptr(slot, field.offset, "field_ptr")?;
                        let field_ty = basic_ty(
                            context,
                            self.structs,
                            self.enums,
                            self.managed_address_space,
                            &field.ty,
                        )?;
                        builder
                            .build_load(field_ty, field_ptr, &name)
                            .map_err(|e| {
                                CodegenError(format!(
                                    "enum_field @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?
                    }
                };
                self.temps.insert(*out, result);
            }
        }
        Ok(())
    }
}
