use super::*;

impl<'a> FunctionLowerer<'a> {
    /// Lower a fully typed MIR expression, appending its instructions to
    /// the current block and returning the value it evaluates to.
    pub(super) fn lower_expr(&mut self, expr: &mir::Expr) -> lir::Value {
        let ty = &expr.ty;
        match &expr.kind {
            mir::ExprKind::StringConst(id) => lir::Value::Global(self.global_map[id]),
            mir::ExprKind::IntegerLiteral(value) => {
                assert_eq!(
                    *ty,
                    mir::Type::Integer(value.kind()),
                    "an integer literal has its exact constant kind",
                );
                lir::Value::IntegerConst(integer_constant(*value))
            }
            mir::ExprKind::MachineScalarLiteral(value) => {
                lir::Value::MachineScalar(machine_scalar_value(*value))
            }
            mir::ExprKind::BoolLiteral(value) => lir::Value::BoolConst(*value),
            mir::ExprKind::UnitLiteral => self.unit_value(),
            mir::ExprKind::CaughtException => lir::Value::Local(
                self.caught_exception
                    .expect("CaughtException must be dominated by BeginCatch"),
            ),
            mir::ExprKind::TupleLiteral(elements) => {
                let mir::Type::Tuple(element_types) = ty else {
                    unreachable!("a tuple literal has a tuple type")
                };
                assert_eq!(elements.len(), element_types.len(), "tuple literal arity");
                let elements: Vec<lir::Value> = elements
                    .iter()
                    .map(|element| self.lower_expr(element))
                    .collect();
                self.make_aggregate(ty, elements)
            }
            mir::ExprKind::StructInit { struct_id, args } => {
                let field_count = self.module.structs[*struct_id].declared_fields().len();
                assert_eq!(args.len(), field_count, "struct initializer arity");
                let args: Vec<lir::Value> = args.iter().map(|arg| self.lower_expr(arg)).collect();
                self.make_aggregate(ty, args)
            }
            // Exact class allocation. The allocator returns only after the
            // complete payload has been zeroed and registered for precise
            // scanning; typed initializer calls perform all field stores.
            mir::ExprKind::ClassAlloc { class_id } => {
                let def = &self.module.classes[*class_id];
                let (_, size, _) = class_shape(self.context, self.module, self.enums, def);
                let td = self.td_ref(&mir::Type::Class(*class_id));
                self.emit_plain_call(
                    LoweredCallDestination::managed_runtime(lir::ManagedRuntimeFunction::Alloc),
                    vec![
                        lir::METADATA_PTR,
                        lir::LirType::MachineScalar(lir::MachineScalarKind::ByteSize),
                    ],
                    lir::MANAGED_PTR,
                    vec![
                        td,
                        lir::Value::MachineScalar(lir::MachineScalarValue::ByteSize(size)),
                    ],
                )
            }
            mir::ExprKind::ClosureAlloc { class, captures } => {
                let def = &self.module.closure_classes[*class];
                let (capture_offsets, size, _, _) =
                    closure_shape(self.context, self.module, self.enums, def);
                assert_eq!(
                    captures.len(),
                    def.captures.len(),
                    "ClosureAlloc initializes every capture field"
                );
                let td = lir::Value::TypeDescriptor(self.type_descriptors.for_closure(*class));
                let object = self.emit_plain_call(
                    LoweredCallDestination::managed_runtime(lir::ManagedRuntimeFunction::Alloc),
                    vec![
                        lir::METADATA_PTR,
                        lir::LirType::MachineScalar(lir::MachineScalarKind::ByteSize),
                    ],
                    lir::MANAGED_PTR,
                    vec![
                        td,
                        lir::Value::MachineScalar(lir::MachineScalarValue::ByteSize(size)),
                    ],
                );
                let invoke_function = self.module.closure_invoke_functions[def.invoke].function;
                let invoke_symbol = self.module.functions[invoke_function].symbol.clone();
                let invoke = self.new_temp(lir::CODE_PTR);
                self.push(lir::Instruction::FunctionAddress {
                    out: invoke,
                    symbol: invoke_symbol,
                });
                let (invoke_offset, _) = self.context.closure_prefix();
                self.store_at_offset(
                    object,
                    invoke_offset,
                    lir::Value::Temp(invoke),
                    lir::CODE_PTR,
                );
                let capture_types = def
                    .captures
                    .iter()
                    .map(|capture| capture.ty.clone())
                    .collect::<Vec<_>>();
                for ((capture, capture_ty), offset) in
                    captures.iter().zip(&capture_types).zip(capture_offsets)
                {
                    assert_eq!(
                        &capture.ty, capture_ty,
                        "ClosureAlloc capture type matches its storage field"
                    );
                    let capture_lir_ty = self.value_type(capture_ty);
                    let value = self.lower_expr(capture);
                    self.store_at_offset(object, offset, value, capture_lir_ty);
                }
                object
            }
            mir::ExprKind::ClosureCapture {
                closure,
                class,
                index,
            } => {
                let def = &self.module.closure_classes[*class];
                let capture_ty = &def.captures[*index as usize].ty;
                assert_eq!(
                    ty, capture_ty,
                    "a closure capture read has its declared storage type"
                );
                let (capture_offsets, _, _, _) =
                    closure_shape(self.context, self.module, self.enums, def);
                let closure = self.lower_expr(closure);
                let out_ty = self.value_type(ty);
                let out = self.load_at_offset(closure, capture_offsets[*index as usize], out_ty);
                lir::Value::Temp(out)
            }
            // Every operation consumes the exact array application carried by
            // MIR. The LIR value itself is just a managed pointer.
            mir::ExprKind::ArrayLiteral {
                array_type,
                elements,
            } => {
                let elements: Vec<lir::Value> = elements
                    .iter()
                    .map(|element| self.lower_expr(element))
                    .collect();
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                let safepoint = self.next_safepoint();
                self.push(lir::Instruction::ArrayAlloc {
                    out,
                    elements,
                    array_type: self.array_type_id(*array_type),
                    safepoint,
                    live: lir::StatepointLiveSet::default(),
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::ArrayAssembly { array_type, parts } => {
                let parts = parts
                    .iter()
                    .map(|part| match part {
                        mir::ArrayAssemblyPart::Element(value) => {
                            lir::ArrayAssemblyPart::Element(self.lower_expr(value))
                        }
                        mir::ArrayAssemblyPart::CopyArray(value) => {
                            lir::ArrayAssemblyPart::CopyArray(self.lower_expr(value))
                        }
                    })
                    .collect();
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                let safepoint = self.next_safepoint();
                self.push(lir::Instruction::ArrayAssembly {
                    out,
                    parts,
                    array_type: self.array_type_id(*array_type),
                    safepoint,
                    live: lir::StatepointLiveSet::default(),
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::ArrayGet {
                array_type,
                array,
                index,
            } => {
                let array = self.lower_expr(array);
                let index = self.lower_expr(index);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::ArrayGet {
                    out,
                    array,
                    index,
                    array_type: self.array_type_id(*array_type),
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::ArrayLen {
                array_type,
                operand,
            } => {
                assert_eq!(
                    *ty,
                    mir::Type::Integer(mir::IntegerKind::SIGNED_64),
                    "array length is canonical Long",
                );
                let operand = self.lower_expr(operand);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::ArrayLen {
                    out,
                    operand,
                    array_type: self.array_type_id(*array_type),
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::ArrayClone {
                source_type: _,
                target_type,
                operand,
            } => {
                let operand = self.lower_expr(operand);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                let safepoint = self.next_safepoint();
                self.push(lir::Instruction::ArrayClone {
                    out,
                    operand,
                    array_type: self.array_type_id(*target_type),
                    safepoint,
                    live: lir::StatepointLiveSet::default(),
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::Local(local) => self.local_value(*local),
            mir::ExprKind::GlobalRead(global) => {
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                match *self
                    .storage_globals
                    .get(global)
                    .expect("every MIR global has storage")
                {
                    StorageGlobal::Local(global) => {
                        self.push(lir::Instruction::GlobalLoad { out, global })
                    }
                    StorageGlobal::Native(global) => {
                        let safepoint = self.next_safepoint();
                        self.push(lir::Instruction::NativeGlobalLoad {
                            out,
                            global,
                            safepoint,
                            roots: lir::NativeSafeRootSet::default(),
                        })
                    }
                }
                lir::Value::Temp(out)
            }
            mir::ExprKind::InitializationUnitAddress(unit) => {
                lir::Value::InitializationUnit(lir::InitializationUnitId::from_raw(unit.into_raw()))
            }
            mir::ExprKind::PtrFromNonZeroULong { operand, pointee } => {
                assert_eq!(
                    operand.ty,
                    mir::Type::Integer(mir::IntegerKind::UNSIGNED_64),
                    "raw pointer construction consumes canonical ULong",
                );
                let mir::Type::Ptr(result_pointee) = ty else {
                    panic!("PtrFromNonZeroULong result must be a raw pointer")
                };
                assert_eq!(
                    result_pointee.as_ref(),
                    pointee.as_ref(),
                    "PtrFromNonZeroULong result carries its declared pointee",
                );
                let value = self.lower_expr(operand);
                let out = self.new_temp(lir::RAW_PTR);
                self.push(lir::Instruction::ULongToPtr { out, value });
                lir::Value::Temp(out)
            }
            mir::ExprKind::PtrToULong(operand) => {
                assert_eq!(
                    *ty,
                    mir::Type::Integer(mir::IntegerKind::UNSIGNED_64),
                    "raw pointer bits are exposed as canonical ULong",
                );
                assert!(
                    matches!(&operand.ty, mir::Type::Ptr(_)),
                    "PtrToULong consumes only a raw data pointer",
                );
                let value = self.lower_expr(operand);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::PtrToULong { out, value });
                lir::Value::Temp(out)
            }
            mir::ExprKind::PtrCast { operand, pointee } => {
                let mir::Type::Ptr(result_pointee) = ty else {
                    panic!("PtrCast result must remain a raw pointer")
                };
                assert_eq!(
                    result_pointee.as_ref(),
                    pointee.as_ref(),
                    "PtrCast result carries its declared pointee"
                );
                assert!(
                    matches!(&operand.ty, mir::Type::Ptr(_)),
                    "PtrCast operand must be a raw pointer"
                );
                self.lower_expr(operand)
            }
            mir::ExprKind::PtrLoad {
                pointer,
                pointee,
                offset,
            } => {
                assert_eq!(
                    ty,
                    pointee.as_ref(),
                    "PtrLoad result must match its pointee"
                );
                let mir::Type::Ptr(pointer_pointee) = &pointer.ty else {
                    panic!("PtrLoad operand must be a raw pointer")
                };
                assert_eq!(
                    pointer_pointee.as_ref(),
                    pointee.as_ref(),
                    "PtrLoad operand pointer must have the declared pointee"
                );
                let pointer = self.lower_expr(pointer);
                let pointer = if let Some(offset) = offset {
                    let offset = self.lower_expr(offset);
                    self.offset_pointer(pointer, pointee, offset, false)
                } else {
                    pointer
                };
                let (_, align) = self.value_layout(pointee);
                let out_ty = self.value_type(pointee);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::RawLoad {
                    out,
                    pointer,
                    align,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::PtrStore {
                pointer,
                pointee,
                offset,
                value,
            } => {
                let mir::Type::Ptr(pointer_pointee) = &pointer.ty else {
                    panic!("PtrStore operand must be a raw pointer")
                };
                assert_eq!(
                    pointer_pointee.as_ref(),
                    pointee.as_ref(),
                    "PtrStore operand pointer must have the declared pointee"
                );
                assert_eq!(
                    &value.ty,
                    pointee.as_ref(),
                    "PtrStore value must match its pointee"
                );
                let pointer = self.lower_expr(pointer);
                let pointer = if let Some(offset) = offset {
                    let offset = self.lower_expr(offset);
                    self.offset_pointer(pointer, pointee, offset, false)
                } else {
                    pointer
                };
                let value = self.lower_expr(value);
                let (_, align) = self.value_layout(pointee);
                self.push(lir::Instruction::RawStore {
                    pointer,
                    value,
                    align,
                });
                self.unit_value()
            }
            mir::ExprKind::PtrOffset {
                pointer,
                pointee,
                offset,
                subtract,
            } => {
                let pointer = self.lower_expr(pointer);
                let offset = self.lower_expr(offset);
                self.offset_pointer(pointer, pointee, offset, *subtract)
            }
            mir::ExprKind::AddressOf { local, .. } => {
                let local = self.local_slot(*local);
                let out = self.new_temp(lir::RAW_PTR);
                self.push(lir::Instruction::LocalAddress { out, local });
                lir::Value::Temp(out)
            }
            mir::ExprKind::GlobalAddress { global, .. } => {
                let out = self.new_temp(lir::RAW_PTR);
                match *self
                    .storage_globals
                    .get(global)
                    .expect("every MIR global has storage")
                {
                    StorageGlobal::Local(global) => {
                        self.push(lir::Instruction::GlobalAddress { out, global })
                    }
                    StorageGlobal::Native(global) => {
                        let safepoint = self.next_safepoint();
                        self.push(lir::Instruction::NativeGlobalAddress {
                            out,
                            global,
                            safepoint,
                            roots: lir::NativeSafeRootSet::default(),
                        })
                    }
                }
                lir::Value::Temp(out)
            }
            mir::ExprKind::SizeOf(value_ty) => {
                assert_eq!(
                    *ty,
                    mir::Type::Integer(mir::IntegerKind::UNSIGNED_64),
                    "sizeOf produces canonical ULong",
                );
                let (size, _) = self.value_layout(value_ty);
                lir::Value::IntegerConst(lir::LirIntegerConstant::Unsigned64(size))
            }
            mir::ExprKind::AlignOf(value_ty) => {
                assert_eq!(
                    *ty,
                    mir::Type::Integer(mir::IntegerKind::UNSIGNED_64),
                    "alignOf produces canonical ULong",
                );
                let (_, align) = self.value_layout(value_ty);
                lir::Value::IntegerConst(lir::LirIntegerConstant::Unsigned64(align))
            }
            mir::ExprKind::FunctionAddress { callback } => {
                let out = self.new_temp(lir::CODE_PTR);
                self.push(lir::Instruction::FunctionAddress {
                    out,
                    symbol: format!("scoop_c_callback_{}", callback.into_raw().into_u32()),
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::ForeignCallbackRegister { bridge, closure } => {
                let closure = self.lower_expr(closure);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::ForeignCallbackRegister {
                    out,
                    bridge: la_arena::Idx::from_raw(bridge.into_raw()),
                    closure,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::ForeignCallbackOperation {
                operation,
                callback,
                ..
            } => {
                let callback = self.lower_expr(callback);
                match operation {
                    mir::ForeignCallbackOperation::Release(family) => {
                        self.push(lir::Instruction::ForeignCallbackOperation(
                            lir::ForeignCallbackOperation::Release {
                                family: lir::ForeignCallbackFamilyId::from_raw(family.into_raw()),
                                callback,
                            },
                        ));
                        self.unit_value()
                    }
                    mir::ForeignCallbackOperation::Retain(family) => {
                        let out_ty = self.value_type(ty);
                        let out = self.new_temp(out_ty);
                        self.push(lir::Instruction::ForeignCallbackOperation(
                            lir::ForeignCallbackOperation::Retain {
                                family: lir::ForeignCallbackFamilyId::from_raw(family.into_raw()),
                                out,
                                callback,
                            },
                        ));
                        lir::Value::Temp(out)
                    }
                    mir::ForeignCallbackOperation::State(family) => {
                        let out_ty = self.value_type(ty);
                        let out = self.new_temp(out_ty);
                        self.push(lir::Instruction::ForeignCallbackOperation(
                            lir::ForeignCallbackOperation::State {
                                family: lir::ForeignCallbackFamilyId::from_raw(family.into_raw()),
                                out,
                                callback,
                            },
                        ));
                        lir::Value::Temp(out)
                    }
                    mir::ForeignCallbackOperation::Failure(family) => {
                        let out_ty = self.value_type(ty);
                        let out = self.new_temp(out_ty);
                        self.push(lir::Instruction::ForeignCallbackOperation(
                            lir::ForeignCallbackOperation::Failure {
                                family: lir::ForeignCallbackFamilyId::from_raw(family.into_raw()),
                                out,
                                callback,
                            },
                        ));
                        lir::Value::Temp(out)
                    }
                }
            }
            mir::ExprKind::Retype {
                operand,
                ty: retyped_ty,
            } => {
                assert_eq!(
                    ty,
                    retyped_ty.as_ref(),
                    "Retype expression carries one result type"
                );
                assert_eq!(
                    lir_type(&operand.ty),
                    lir::MANAGED_PTR,
                    "Retype operand must be a managed reference"
                );
                assert_eq!(
                    lir_type(ty),
                    lir::MANAGED_PTR,
                    "Retype result must be a managed reference"
                );
                self.lower_expr(operand)
            }
            mir::ExprKind::FieldAccess { receiver, index } => {
                let receiver_ty = receiver.ty.clone();
                if let mir::Type::Class(class_id) = &receiver_ty {
                    let field_ty =
                        &self.module.classes[*class_id].declared_fields()[*index as usize].ty;
                    assert_eq!(ty, field_ty, "a field access has its declared field type");
                }
                let receiver = self.lower_expr(receiver);
                let out_ty = self.value_type(ty);
                let out = if let mir::Type::Class(class_id) = receiver_ty {
                    let (offsets, _, _) = class_shape(
                        self.context,
                        self.module,
                        self.enums,
                        &self.module.classes[class_id],
                    );
                    self.load_at_offset(receiver, offsets[*index as usize], out_ty)
                } else {
                    let out = self.new_temp(out_ty);
                    self.push(lir::Instruction::ExtractValue {
                        out,
                        aggregate: receiver,
                        index: *index,
                    });
                    out
                };
                lir::Value::Temp(out)
            }
            mir::ExprKind::AtomicFieldLoad {
                kind,
                object,
                index,
            } => {
                let object_ty = object.ty.clone();
                let mir::Type::Class(class_id) = &object_ty else {
                    unreachable!("an atomic field load targets a class object")
                };
                assert!(kind.is_atomic_state(), "only coroutine state is atomic");
                assert_eq!(
                    self.module.classes[*class_id].declared_fields()[*index as usize].ty,
                    mir::Type::MachineScalar(*kind),
                    "an atomic state operation must match its field domain"
                );
                let (offsets, _, _) = class_shape(
                    self.context,
                    self.module,
                    self.enums,
                    &self.module.classes[*class_id],
                );
                let object = self.lower_expr(object);
                let kind = machine_scalar_kind(*kind);
                let out = self.new_temp(lir::LirType::MachineScalar(kind));
                self.push(lir::Instruction::AtomicLoad {
                    out,
                    kind,
                    object,
                    offset: offsets[*index as usize],
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::AtomicFieldCompareExchange {
                kind,
                object,
                index,
                expected,
                replacement,
            } => {
                let object_ty = object.ty.clone();
                let mir::Type::Class(class_id) = &object_ty else {
                    unreachable!("an atomic compare-exchange targets a class object")
                };
                assert!(kind.is_atomic_state(), "only coroutine state is atomic");
                assert_eq!(
                    self.module.classes[*class_id].declared_fields()[*index as usize].ty,
                    mir::Type::MachineScalar(*kind),
                    "an atomic state operation must match its field domain"
                );
                assert_eq!(
                    expected.ty,
                    mir::Type::MachineScalar(*kind),
                    "atomic expected value must match its field domain"
                );
                assert_eq!(
                    replacement.ty,
                    mir::Type::MachineScalar(*kind),
                    "atomic replacement value must match its field domain"
                );
                let (offsets, _, _) = class_shape(
                    self.context,
                    self.module,
                    self.enums,
                    &self.module.classes[*class_id],
                );
                let object = self.lower_expr(object);
                let expected = self.lower_expr(expected);
                let replacement = self.lower_expr(replacement);
                let kind = machine_scalar_kind(*kind);
                let out = self.new_temp(lir::LirType::MachineScalar(kind));
                self.push(lir::Instruction::AtomicCompareExchange {
                    out,
                    kind,
                    object,
                    offset: offsets[*index as usize],
                    expected,
                    replacement,
                });
                lir::Value::Temp(out)
            }
            // `scoop_rt_box(td, payload, size, scan)` (runtime spec 2.3): LIR
            // materializes the payload storage and carries its complete
            // recursive scan program into the managed runtime entry.
            mir::ExprKind::Box(operand) => {
                let payload_ty = operand.ty.clone();
                record_layout_types(&payload_ty, self.layout_types);
                let payload = self.lower_expr(operand);
                let payload_lir_type = self.value_type(&payload_ty);
                let payload_storage = self.new_hidden_local(payload_lir_type);
                self.push(lir::Instruction::Store {
                    local: payload_storage,
                    value: payload,
                });
                let payload_address = self.new_temp(lir::RAW_PTR);
                self.push(lir::Instruction::LocalAddress {
                    out: payload_address,
                    local: payload_storage,
                });
                let td = self.td_ref(&payload_ty);
                let (size, _) = self.value_layout(&payload_ty);
                let payload_scan = self
                    .call_targets
                    .root_scans
                    .alloc(self.value_ref_scan(&payload_ty));
                self.emit_plain_call(
                    LoweredCallDestination::managed_runtime(lir::ManagedRuntimeFunction::Box),
                    vec![
                        lir::METADATA_PTR,
                        lir::RAW_PTR,
                        lir::LirType::MachineScalar(lir::MachineScalarKind::ByteSize),
                        lir::METADATA_PTR,
                    ],
                    lir::MANAGED_PTR,
                    vec![
                        td,
                        lir::Value::Temp(payload_address),
                        lir::Value::MachineScalar(lir::MachineScalarValue::ByteSize(size)),
                        lir::Value::RootScan(payload_scan),
                    ],
                )
            }
            // The payload follows the target-derived object header at its
            // own natural alignment.
            mir::ExprKind::Unbox(operand) => {
                let object = self.lower_expr(operand);
                let (_, payload_align) = self.value_layout(ty);
                let payload_offset = self.context.object_payload_offset(payload_align);
                let ty = self.value_type(ty);
                let out = self.load_at_offset(object, payload_offset, ty);
                lir::Value::Temp(out)
            }
            // `scoop_rt_is_instance(obj, td)` (runtime spec 2.3).
            mir::ExprKind::IsInstance { operand, check_ty } => {
                let object = self.lower_expr(operand);
                let td = self.td_ref(check_ty);
                self.emit_plain_call(
                    LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::IsInstance),
                    vec![lir::MANAGED_PTR, lir::METADATA_PTR],
                    lir::LirType::I1,
                    vec![object, td],
                )
            }
            // mir-lower expands `as` / `as?` into runtime checks plus
            // Option wrapping; the node never reaches LIR.
            mir::ExprKind::Cast { .. } => unreachable!("mir-lower expands casts before LIR"),
            // The enum operations map onto the corresponding LIR
            // instructions; the concrete representation (niche pointer
            // or tagged union) is fixed by the `EnumDef`, so codegen
            // translates them mechanically.
            mir::ExprKind::VariantConstruct { variant, fields } => {
                let mir::Type::Enum(enum_id, _) = ty else {
                    unreachable!("a variant construction has an enum type")
                };
                let field_count = self.module.enums[*enum_id].variants[*variant as usize]
                    .fields
                    .len();
                assert_eq!(fields.len(), field_count, "enum variant field arity");
                let fields: Vec<lir::Value> =
                    fields.iter().map(|field| self.lower_expr(field)).collect();
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::EnumWrap {
                    out,
                    enum_id: enum_def_id(*enum_id),
                    variant: *variant,
                    fields,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::EnumTag(operand) => {
                let operand_ty = operand.ty.clone();
                let mir::Type::Enum(enum_id, _) = &operand_ty else {
                    unreachable!("a tag read's operand is an enum value")
                };
                let operand = self.lower_expr(operand);
                assert_eq!(
                    *ty,
                    mir::Type::MachineScalar(mir::MachineScalarKind::EnumTag),
                    "an enum tag read has the internal enum-tag domain"
                );
                let out =
                    self.new_temp(lir::LirType::MachineScalar(lir::MachineScalarKind::EnumTag));
                self.push(lir::Instruction::EnumTag {
                    out,
                    enum_id: enum_def_id(*enum_id),
                    operand,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::EnumField {
                operand,
                variant,
                index,
            } => {
                let operand_ty = operand.ty.clone();
                let mir::Type::Enum(enum_id, _) = &operand_ty else {
                    unreachable!("an enum field read's operand is an enum value")
                };
                let enum_id = *enum_id;
                let operand = self.lower_expr(operand);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::EnumField {
                    out,
                    enum_id: enum_def_id(enum_id),
                    variant: *variant,
                    index: *index,
                    operand,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::Binary { op, lhs, rhs } => {
                let lir_op = binary_op(*op);
                let lhs = self.lower_expr(lhs);
                let rhs = self.lower_expr(rhs);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::BinOp {
                    out,
                    op: lir_op,
                    lhs,
                    rhs,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::Unary { op, operand } => {
                let lir_op = match op {
                    mir::UnOp::BoolNot => lir::UnOp::Not,
                };
                let operand = self.lower_expr(operand);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::UnaryOp {
                    out,
                    op: lir_op,
                    operand,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::IntegerUnary { operation, operand } => {
                let kind = integer_kind(operation.kind());
                assert_eq!(
                    *ty,
                    mir::Type::Integer(operation.kind()),
                    "integer unary result preserves its exact kind",
                );
                let operand = self.lower_expr(operand);
                let out = self.new_temp(kind.scalar_type());
                match operation.operator() {
                    mir::IntegerUnaryOperator::Identity => {
                        self.push(lir::Instruction::IntegerUnary {
                            out,
                            kind,
                            operation: lir::IntegerUnaryOperation::Plus,
                            operand,
                        });
                    }
                    mir::IntegerUnaryOperator::Negate => {
                        self.push(lir::Instruction::IntegerUnary {
                            out,
                            kind,
                            operation: lir::IntegerUnaryOperation::Negate,
                            operand,
                        });
                    }
                    mir::IntegerUnaryOperator::BitNot => {
                        self.push(lir::Instruction::IntegerUnary {
                            out,
                            kind,
                            operation: lir::IntegerUnaryOperation::BitwiseNot,
                            operand,
                        });
                    }
                    mir::IntegerUnaryOperator::Increment | mir::IntegerUnaryOperator::Decrement => {
                        let one = mir::MirIntegerConstant::from_raw_bits(operation.kind(), 1)
                            .expect("one is representable by every integer kind");
                        self.push(lir::Instruction::IntegerBinary {
                            out,
                            kind,
                            operation: if operation.operator()
                                == mir::IntegerUnaryOperator::Increment
                            {
                                lir::IntegerBinaryOperation::Add
                            } else {
                                lir::IntegerBinaryOperation::Subtract
                            },
                            lhs: operand,
                            rhs: lir::Value::IntegerConst(integer_constant(one)),
                        });
                    }
                }
                lir::Value::Temp(out)
            }
            mir::ExprKind::IntegerBinary {
                operation,
                lhs,
                rhs,
            } => {
                let kind = integer_kind(operation.kind());
                assert_eq!(
                    *ty,
                    mir::Type::Integer(operation.kind()),
                    "integer binary result preserves its exact kind",
                );
                let lhs = self.lower_expr(lhs);
                let rhs = self.lower_expr(rhs);
                let out = self.new_temp(kind.scalar_type());
                self.push(lir::Instruction::IntegerBinary {
                    out,
                    kind,
                    operation: integer_binary_op(operation.operator()),
                    lhs,
                    rhs,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::SafeIntegerDivRem {
                operation,
                lhs,
                rhs,
            } => {
                let kind = integer_kind(operation.kind());
                assert_eq!(
                    *ty,
                    mir::Type::Integer(operation.kind()),
                    "safe integer div/rem result preserves its exact kind",
                );
                let lhs = self.lower_expr(lhs);
                let rhs = self.lower_expr(rhs);
                let out = self.new_temp(kind.scalar_type());
                self.push(lir::Instruction::SafeIntegerDivRem {
                    out,
                    kind,
                    operation: integer_div_rem_op(operation.operator()),
                    lhs,
                    rhs,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::IntegerCompare {
                operation,
                lhs,
                rhs,
            } => {
                assert_eq!(
                    *ty,
                    mir::Type::Boolean,
                    "integer comparison returns Boolean"
                );
                let lhs = self.lower_expr(lhs);
                let rhs = self.lower_expr(rhs);
                let out = self.new_temp(lir::LirType::I1);
                self.push(lir::Instruction::IntegerCompare {
                    out,
                    kind: integer_kind(operation.operand_kind()),
                    comparison: integer_comparison(operation.operator()),
                    lhs,
                    rhs,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::IntegerCompareTo {
                operation,
                lhs,
                rhs,
            } => {
                assert_eq!(
                    *ty,
                    mir::Type::Integer(mir::IntegerKind::SIGNED_64),
                    "integer compareTo returns canonical Long",
                );
                let lhs = self.lower_expr(lhs);
                let rhs = self.lower_expr(rhs);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::IntegerCompareTo {
                    out,
                    operand_kind: integer_kind(operation.operand_kind()),
                    lhs,
                    rhs,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::IntegerShift {
                operation,
                value,
                count,
            } => {
                assert_eq!(
                    *ty,
                    mir::Type::Integer(operation.value_kind()),
                    "integer shift preserves its exact value kind",
                );
                assert_eq!(
                    count.ty,
                    mir::Type::Integer(operation.count_kind()),
                    "integer shift count is normalized to the value's exact kind",
                );
                let value = self.lower_expr(value);
                let count = self.lower_expr(count);
                let kind = integer_kind(operation.value_kind());
                let out = self.new_temp(kind.scalar_type());
                self.push(lir::Instruction::IntegerShift {
                    out,
                    kind,
                    operation: integer_shift_op(*operation),
                    value,
                    normalized_count: count,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::IntegerConversion {
                conversion,
                operand,
            } => {
                assert_eq!(
                    *ty,
                    mir::Type::Integer(conversion.target_kind()),
                    "integer conversion has its exact target kind",
                );
                let operand = self.lower_expr(operand);
                let target_kind = integer_kind(conversion.target_kind());
                let out = self.new_temp(target_kind.scalar_type());
                self.push(lir::Instruction::IntegerConvert {
                    out,
                    source_kind: integer_kind(conversion.source_kind()),
                    target_kind,
                    operand,
                });
                lir::Value::Temp(out)
            }
        }
    }

    pub(super) fn offset_pointer(
        &mut self,
        pointer: lir::Value,
        pointee: &mir::Type,
        offset: lir::Value,
        subtract: bool,
    ) -> lir::Value {
        let (size, _) = self.value_layout(pointee);
        let out = self.new_temp(lir::RAW_PTR);
        self.push(lir::Instruction::PtrOffset {
            out,
            pointer,
            element_offset: offset,
            element_size: size,
            subtract,
        });
        lir::Value::Temp(out)
    }

    /// The shared trap block for `message` in this function (one per
    /// message, created on first use): calls the runtime trap —
    /// `void scoop_rt_trap(ptr)`, noreturn — with the message global
    /// and ends `unreachable`.
    pub(super) fn trap_block(&mut self, message: &str) -> lir::BlockId {
        if let Some(&block) = self.trap_blocks.get(message) {
            return block;
        }
        let symbol = format!("scoop.cstr.{}", *self.cstr_count);
        *self.cstr_count += 1;
        let global = self.globals.alloc(lir::Global {
            symbol,
            address_kind: lir::PointerKind::Raw,
            scan: lir::RefScan::None,
            init: lir::GlobalInit::CString(message.to_string()),
        });
        let block = self.new_block("unwrap.trap");
        // Fill the trap block out of line; the caller seals the
        // suspended current block with the branch.
        let saved = self.current;
        let saved_sealed = self.current_sealed;
        self.enter(block);
        let (call, result) = self.typed_call(
            vec![lir::RAW_PTR],
            lir::LirType::Void,
            vec![lir::Value::Global(global)],
        );
        assert!(result.is_none(), "trap has no value result");
        let site = self.call_site(
            LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::Trap),
            call,
        );
        self.push(lir::Instruction::Call { site });
        self.seal(lir::Terminator::Unreachable);
        self.trap_blocks.insert(message.to_string(), block);
        self.current = saved;
        self.current_sealed = saved_sealed;
        block
    }

    pub(super) fn td_ref(&self, ty: &mir::Type) -> lir::Value {
        lir::Value::TypeDescriptor(self.type_descriptors.for_type(ty))
    }

    /// Struct / tuple construction: an aggregate of the mapped field
    /// values in declaration order.
    pub(super) fn make_aggregate(
        &mut self,
        ty: &mir::Type,
        elements: Vec<lir::Value>,
    ) -> lir::Value {
        let ty = self.value_type(ty);
        let out = self.new_temp(ty);
        self.push(lir::Instruction::MakeAggregate { out, elements });
        lir::Value::Temp(out)
    }

    /// The Unit value: an empty aggregate (void calls and Unit
    /// literals produce it; void functions never return it).
    pub(super) fn unit_value(&mut self) -> lir::Value {
        let out = self.new_temp(lir::LirType::Aggregate(Vec::new()));
        self.push(lir::Instruction::MakeAggregate {
            out,
            elements: Vec::new(),
        });
        lir::Value::Temp(out)
    }
}
