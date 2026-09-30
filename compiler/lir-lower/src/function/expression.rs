use super::*;

impl<'a> FunctionLowerer<'a> {
    /// Lower a fully typed MIR expression, appending its instructions to
    /// the current block and returning the value it evaluates to.
    pub(super) fn lower_expr(&mut self, expr: &mir::Expr) -> StorageResult<lir::Value> {
        let ty = &expr.ty;
        Ok(match &expr.kind {
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
                    .collect::<StorageResult<Vec<_>>>()?;
                self.make_aggregate(ty, elements)
            }
            mir::ExprKind::StructInit { struct_id, args } => {
                let field_count = self.module.structs[*struct_id].declared_fields().len();
                assert_eq!(args.len(), field_count, "struct initializer arity");
                let args: Vec<lir::Value> = args
                    .iter()
                    .map(|arg| self.lower_expr(arg))
                    .collect::<StorageResult<Vec<_>>>()?;
                self.make_aggregate(ty, args)
            }
            mir::ExprKind::StructConstruct { struct_id, fields } => {
                let field_count = self.module.structs[*struct_id].declared_fields().len();
                assert_eq!(fields.len(), field_count, "raw struct construction arity");
                let fields: Vec<lir::Value> = fields
                    .iter()
                    .map(|field| self.lower_expr(field))
                    .collect::<StorageResult<Vec<_>>>()?;
                self.make_aggregate(ty, fields)
            }
            // Exact class allocation. The allocator returns only after the
            // complete payload has been zeroed and registered for precise
            // scanning; typed initializer calls perform all field stores.
            mir::ExprKind::ClassAlloc { class_id } => self.lower_class_alloc(class_id)?,
            mir::ExprKind::ClosureAlloc { class, captures } => {
                self.lower_closure_alloc(class, captures)?
            }
            mir::ExprKind::ClosureCapture {
                closure,
                class,
                index,
            } => self.lower_closure_capture(ty, closure, class, index)?,
            // Every operation consumes the exact array application carried by
            // MIR. The LIR value itself is just a managed pointer.
            mir::ExprKind::ArrayLiteral {
                array_type,
                elements,
            } => self.lower_array_literal(ty, array_type, elements)?,
            mir::ExprKind::ArrayAssembly { array_type, parts } => {
                self.lower_array_assembly(ty, array_type, parts)?
            }
            mir::ExprKind::ArrayGet {
                array_type,
                array,
                index,
            } => self.lower_array_get(ty, array_type, array, index)?,
            mir::ExprKind::ArrayLen {
                array_type,
                operand,
            } => self.lower_array_len(ty, array_type, operand)?,
            mir::ExprKind::ArrayClone {
                source_type,
                target_type,
                operand,
            } => self.lower_array_clone(ty, source_type, target_type, operand)?,
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
                        let safepoint =
                            self.new_safepoint(lir::SafepointSiteRole::NativeSafeTransition);
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
            mir::ExprKind::PtrFromNonZeroULong { .. }
            | mir::ExprKind::PtrToULong(_)
            | mir::ExprKind::PtrCast { .. }
            | mir::ExprKind::PtrLoad { .. }
            | mir::ExprKind::PtrStore { .. }
            | mir::ExprKind::PtrOffset { .. }
            | mir::ExprKind::AddressOf { .. } => self.lower_pointer_expr(ty, &expr.kind)?,
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
                        let safepoint =
                            self.new_safepoint(lir::SafepointSiteRole::NativeSafeTransition);
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
                let (size, _) = self.value_layout(value_ty)?;
                lir::Value::IntegerConst(lir::LirIntegerConstant::Unsigned64(size))
            }
            mir::ExprKind::AlignOf(value_ty) => {
                assert_eq!(
                    *ty,
                    mir::Type::Integer(mir::IntegerKind::UNSIGNED_64),
                    "alignOf produces canonical ULong",
                );
                let (_, align) = self.value_layout(value_ty)?;
                lir::Value::IntegerConst(lir::LirIntegerConstant::Unsigned64(align))
            }
            mir::ExprKind::FunctionAddress { callback } => {
                let out = self.new_temp(lir::CODE_PTR);
                self.push(lir::Instruction::FunctionAddress {
                    out,
                    target: lir::FunctionAddressTarget::CallbackTrampoline(
                        self.callback_map[callback],
                    ),
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::ForeignCallbackRegister { bridge, closure } => {
                let closure = self.lower_expr(closure)?;
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
                let callback = self.lower_expr(callback)?;
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
                self.lower_expr(operand)?
            }
            mir::ExprKind::FieldAccess { receiver, index } => {
                self.lower_field_access(ty, receiver, index)?
            }
            mir::ExprKind::AtomicFieldLoad {
                kind,
                object,
                index,
            } => self.lower_atomic_field_load(kind, object, index)?,
            mir::ExprKind::AtomicFieldCompareExchange {
                kind,
                object,
                index,
                expected,
                replacement,
            } => self.lower_atomic_field_compare_exchange(
                kind,
                object,
                index,
                expected,
                replacement,
            )?,
            mir::ExprKind::Box(operand) => self.lower_box(operand)?,
            mir::ExprKind::Unbox(operand) => self.lower_unbox(operand, ty)?,
            // `scoop_rt_is_instance(obj, td)` (runtime spec 2.3).
            mir::ExprKind::IsInstance { operand, check_ty } => {
                let object = self.lower_expr(operand)?;
                let td = self.td_ref(check_ty);
                self.emit_plain_call(
                    LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::IsInstance),
                    vec![lir::MANAGED_PTR, lir::METADATA_PTR],
                    lir::LirType::I1,
                    vec![object, td],
                )?
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
                assert_eq!(variant.enum_id(), *enum_id);
                let field_count = self.module.enums[*enum_id].variants
                    [variant.variant_index() as usize]
                    .fields
                    .len();
                assert_eq!(fields.len(), field_count, "enum variant field arity");
                let fields: Vec<lir::Value> = fields
                    .iter()
                    .map(|field| self.lower_expr(field))
                    .collect::<StorageResult<Vec<_>>>()?;
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::EnumWrap {
                    out,
                    variant: variant_ref(self.enums, *variant),
                    fields,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::EnumTag(operand) => {
                let operand_ty = operand.ty.clone();
                let mir::Type::Enum(enum_id, _) = &operand_ty else {
                    unreachable!("a tag read's operand is an enum value")
                };
                let operand = self.lower_expr(operand)?;
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
                let operand = self.lower_expr(operand)?;
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
            mir::ExprKind::VariantTest { operand, variant } => {
                assert_eq!(
                    *ty,
                    mir::Type::Boolean,
                    "a representation-independent variant test produces Boolean"
                );
                let operand = self.lower_expr(operand)?;
                let variant = variant_ref(self.enums, *variant);
                let out = self.new_temp(lir::LirType::I1);
                self.push(lir::Instruction::VariantTest {
                    out,
                    operand,
                    variant,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::VariantPayloadProject { operand, field } => {
                let operand = self.lower_expr(operand)?;
                let field = variant_field_ref(self.enums, *field);
                let out_ty = self.value_type(ty);
                assert_eq!(
                    self.enums.variant_field_type(field),
                    Some(out_ty.clone()),
                    "a representation-independent payload projection keeps its exact field type"
                );
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::VariantPayloadProject {
                    out,
                    operand,
                    field,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::Binary { op, lhs, rhs } => self.lower_binary(ty, op, lhs, rhs)?,
            mir::ExprKind::Unary { op, operand } => self.lower_unary(ty, op, operand)?,
            mir::ExprKind::IntegerUnary { operation, operand } => {
                self.lower_integer_unary(ty, operation, operand)?
            }
            mir::ExprKind::IntegerBinary {
                operation,
                lhs,
                rhs,
            } => self.lower_integer_binary(ty, operation, lhs, rhs)?,
            mir::ExprKind::SafeIntegerDivRem {
                operation,
                lhs,
                rhs,
            } => self.lower_safe_integer_div_rem(ty, operation, lhs, rhs)?,
            mir::ExprKind::IntegerCompare {
                operation,
                lhs,
                rhs,
            } => self.lower_integer_compare(ty, operation, lhs, rhs)?,
            mir::ExprKind::IntegerCompareTo {
                operation,
                lhs,
                rhs,
            } => self.lower_integer_compare_to(ty, operation, lhs, rhs)?,
            mir::ExprKind::IntegerShift {
                operation,
                value,
                count,
            } => self.lower_integer_shift(ty, operation, value, count)?,
            mir::ExprKind::IntegerConversion {
                conversion,
                operand,
            } => self.lower_integer_conversion(ty, conversion, operand)?,
        })
    }
}
