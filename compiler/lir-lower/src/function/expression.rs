use super::*;

impl<'a> FunctionLowerer<'a> {
    /// Lower a fully typed MIR expression, appending its instructions to
    /// the current block and returning the value it evaluates to.
    pub(super) fn lower_expr(&mut self, expr: &mir::Expr) -> lir::Value {
        let ty = &expr.ty;
        match &expr.kind {
            mir::ExprKind::StringConst(id) => lir::Value::Global(self.global_map[id]),
            mir::ExprKind::IntLiteral(value) => lir::Value::IntConst(*value),
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
            // Raw class construction (only ever inside mir-lower's
            // generated ctor functions): `scoop_rt_alloc(td, size)`,
            // then one heap store per flattened field (the header is
            // followed by naturally aligned fields at fixed byte
            // offsets).
            mir::ExprKind::ClassInit { class_id, args } => {
                let def = &self.module.classes[*class_id];
                let (field_offsets, size, _) = class_shape(self.module, self.enums, def);
                assert_eq!(
                    args.len(),
                    def.declared_fields().len(),
                    "a ClassInit initializes every flattened field"
                );
                let td = self.td_ref(&mir::Type::Class(*class_id));
                let object = self.emit_plain_call(
                    LoweredCallDestination::managed_runtime(lir::ManagedRuntimeFunction::Alloc),
                    vec![lir::METADATA_PTR, lir::LirType::I64],
                    lir::MANAGED_PTR,
                    vec![td, lir::Value::IntConst(size as i64)],
                );
                for (arg, offset) in args.iter().zip(field_offsets) {
                    let value = self.lower_expr(arg);
                    self.push(lir::Instruction::HeapStore {
                        object,
                        offset,
                        value,
                    });
                }
                object
            }
            mir::ExprKind::ClosureAlloc { class, captures } => {
                let def = &self.module.closure_classes[*class];
                let (capture_offsets, size, _, _) = closure_shape(self.module, self.enums, def);
                assert_eq!(
                    captures.len(),
                    def.captures.len(),
                    "ClosureAlloc initializes every capture field"
                );
                let td = lir::Value::TypeDescriptor(self.type_descriptors.for_closure(*class));
                let object = self.emit_plain_call(
                    LoweredCallDestination::managed_runtime(lir::ManagedRuntimeFunction::Alloc),
                    vec![lir::METADATA_PTR, lir::LirType::I64],
                    lir::MANAGED_PTR,
                    vec![td, lir::Value::IntConst(size as i64)],
                );
                let invoke_function = self.module.closure_invoke_functions[def.invoke].function;
                let invoke_symbol = self.module.functions[invoke_function].symbol.clone();
                let invoke = self.new_temp(lir::CODE_PTR);
                self.push(lir::Instruction::FunctionAddress {
                    out: invoke,
                    symbol: invoke_symbol,
                });
                self.push(lir::Instruction::HeapStore {
                    object,
                    offset: 16,
                    value: lir::Value::Temp(invoke),
                });
                for (capture, offset) in captures.iter().zip(capture_offsets) {
                    let value = self.lower_expr(capture);
                    self.push(lir::Instruction::HeapStore {
                        object,
                        offset,
                        value,
                    });
                }
                object
            }
            mir::ExprKind::ClosureCapture {
                closure,
                class,
                index,
            } => {
                let def = &self.module.closure_classes[*class];
                let (capture_offsets, _, _, _) = closure_shape(self.module, self.enums, def);
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
            mir::ExprKind::PtrFromUInt { operand, .. } => {
                let value = self.lower_expr(operand);
                let out = self.new_temp(lir::RAW_PTR);
                self.push(lir::Instruction::IntToPtr { out, value });
                lir::Value::Temp(out)
            }
            mir::ExprKind::PtrToUInt(operand) => {
                let value = self.lower_expr(operand);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::PtrToInt { out, value });
                lir::Value::Temp(out)
            }
            mir::ExprKind::PtrCast { operand, .. } => self.lower_expr(operand),
            mir::ExprKind::PtrLoad {
                pointer,
                pointee,
                offset,
            } => {
                let pointer = self.lower_expr(pointer);
                let pointer = if let Some(offset) = offset {
                    let offset = self.lower_expr(offset);
                    self.offset_pointer(pointer, pointee, offset, false)
                } else {
                    pointer
                };
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (_, align) = size_align(self.module, &enum_shape, pointee);
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
                let pointer = self.lower_expr(pointer);
                let pointer = if let Some(offset) = offset {
                    let offset = self.lower_expr(offset);
                    self.offset_pointer(pointer, pointee, offset, false)
                } else {
                    pointer
                };
                let value = self.lower_expr(value);
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (_, align) = size_align(self.module, &enum_shape, pointee);
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
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (size, _) = size_align(self.module, &enum_shape, value_ty);
                lir::Value::IntConst(size as i64)
            }
            mir::ExprKind::AlignOf(value_ty) => {
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (_, align) = size_align(self.module, &enum_shape, value_ty);
                lir::Value::IntConst(align as i64)
            }
            mir::ExprKind::FunPtrNull(_) => lir::Value::NullPointer(lir::PointerKind::Code),
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
                    mir::ForeignCallbackOperation::Release => {
                        self.push(lir::Instruction::ForeignCallbackOperation(
                            lir::ForeignCallbackOperation::Release { callback },
                        ));
                        self.unit_value()
                    }
                    mir::ForeignCallbackOperation::Retain => {
                        let out_ty = self.value_type(ty);
                        let out = self.new_temp(out_ty);
                        self.push(lir::Instruction::ForeignCallbackOperation(
                            lir::ForeignCallbackOperation::Retain { out, callback },
                        ));
                        lir::Value::Temp(out)
                    }
                    mir::ForeignCallbackOperation::State => {
                        let out_ty = self.value_type(ty);
                        let out = self.new_temp(out_ty);
                        self.push(lir::Instruction::ForeignCallbackOperation(
                            lir::ForeignCallbackOperation::State { out, callback },
                        ));
                        lir::Value::Temp(out)
                    }
                    mir::ForeignCallbackOperation::Failure => {
                        let out_ty = self.value_type(ty);
                        let out = self.new_temp(out_ty);
                        self.push(lir::Instruction::ForeignCallbackOperation(
                            lir::ForeignCallbackOperation::Failure { out, callback },
                        ));
                        lir::Value::Temp(out)
                    }
                }
            }
            mir::ExprKind::Retype { operand, .. } => self.lower_expr(operand),
            mir::ExprKind::FieldAccess { receiver, index } => {
                let receiver_ty = receiver.ty.clone();
                let receiver = self.lower_expr(receiver);
                let out_ty = self.value_type(ty);
                let out = if let mir::Type::Class(class_id) = receiver_ty {
                    let (offsets, _, _) =
                        class_shape(self.module, self.enums, &self.module.classes[class_id]);
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
            mir::ExprKind::AtomicFieldLoad { object, index } => {
                let object_ty = object.ty.clone();
                let mir::Type::Class(class_id) = &object_ty else {
                    unreachable!("an atomic field load targets a class object")
                };
                assert_eq!(
                    self.module.classes[*class_id].declared_fields()[*index as usize].ty,
                    mir::Type::Int,
                    "an atomic state field is a 64-bit Int"
                );
                let (offsets, _, _) =
                    class_shape(self.module, self.enums, &self.module.classes[*class_id]);
                let object = self.lower_expr(object);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::AtomicLoad {
                    out,
                    object,
                    offset: offsets[*index as usize],
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::AtomicFieldCompareExchange {
                object,
                index,
                expected,
                replacement,
            } => {
                let object_ty = object.ty.clone();
                let mir::Type::Class(class_id) = &object_ty else {
                    unreachable!("an atomic compare-exchange targets a class object")
                };
                assert_eq!(
                    self.module.classes[*class_id].declared_fields()[*index as usize].ty,
                    mir::Type::Int,
                    "an atomic state field is a 64-bit Int"
                );
                let (offsets, _, _) =
                    class_shape(self.module, self.enums, &self.module.classes[*class_id]);
                let object = self.lower_expr(object);
                let expected = self.lower_expr(expected);
                let replacement = self.lower_expr(replacement);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::AtomicCompareExchange {
                    out,
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
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (size, _) = size_align(self.module, &enum_shape, &payload_ty);
                let payload_scan = self.call_targets.root_scans.alloc(ref_scan(
                    self.module,
                    self.enums,
                    &payload_ty,
                    0,
                ));
                self.emit_plain_call(
                    LoweredCallDestination::managed_runtime(lir::ManagedRuntimeFunction::Box),
                    vec![
                        lir::METADATA_PTR,
                        lir::RAW_PTR,
                        lir::LirType::I64,
                        lir::METADATA_PTR,
                    ],
                    lir::MANAGED_PTR,
                    vec![
                        td,
                        lir::Value::Temp(payload_address),
                        lir::Value::IntConst(size as i64),
                        lir::Value::RootScan(payload_scan),
                    ],
                )
            }
            // The payload sits right behind the 16-byte object header:
            // byte offset 16 of the boxed object (see the module docs).
            mir::ExprKind::Unbox(operand) => {
                let object = self.lower_expr(operand);
                let ty = self.value_type(ty);
                let out = self.load_at_offset(object, 16, ty);
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
                let out = self.new_temp(lir::LirType::I64);
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
                    mir::UnOp::IntNeg => lir::UnOp::Neg,
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
        }
    }

    pub(super) fn offset_pointer(
        &mut self,
        pointer: lir::Value,
        pointee: &mir::Type,
        offset: lir::Value,
        subtract: bool,
    ) -> lir::Value {
        let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
        let (size, _) = size_align(self.module, &enum_shape, pointee);
        let bytes = if size == 1 {
            offset
        } else {
            let out = self.new_temp(lir::LirType::I64);
            self.push(lir::Instruction::BinOp {
                out,
                op: lir::BinOp::Mul,
                lhs: offset,
                rhs: lir::Value::IntConst(size as i64),
            });
            lir::Value::Temp(out)
        };
        let bytes = if subtract {
            let out = self.new_temp(lir::LirType::I64);
            self.push(lir::Instruction::UnaryOp {
                out,
                op: lir::UnOp::Neg,
                operand: bytes,
            });
            lir::Value::Temp(out)
        } else {
            bytes
        };
        let out = self.new_temp(lir::RAW_PTR);
        self.push(lir::Instruction::PtrOffset {
            out,
            pointer,
            bytes,
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
