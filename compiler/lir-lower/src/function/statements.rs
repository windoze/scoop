//! MIR statement, exception-region, and terminator lowering.

use super::*;

impl<'a> FunctionLowerer<'a> {
    pub(super) fn lower_statements(
        &mut self,
        statements: &'a [mir::Statement],
    ) -> StorageResult<()> {
        for statement in statements {
            if self.current_sealed {
                break;
            }
            self.lower_statement(statement)?;
        }
        Ok(())
    }

    fn lower_statement(&mut self, statement: &'a mir::Statement) -> StorageResult<()> {
        match &statement.kind {
            mir::StatementKind::Expr(expr) => {
                self.lower_expr(expr)?;
            }
            mir::StatementKind::Call(effect) => match effect {
                mir::CallEffect::Unit(call) => {
                    let _ = self.lower_call(call, &mir::Type::Unit)?;
                }
                mir::CallEffect::Value { destination, call } => {
                    let ty = self.mir_locals[*destination].ty.clone();
                    let value = self.lower_call(call, &ty)?;
                    self.push(lir::Instruction::Store {
                        local: self.local_slot(*destination),
                        value,
                    });
                }
            },
            // Initialization and assignment are both stores into the
            // local's stack slot.
            mir::StatementKind::ValDecl { local, init } => {
                let value = self.lower_expr(init)?;
                self.push(lir::Instruction::Store {
                    local: self.local_slot(*local),
                    value,
                });
            }
            mir::StatementKind::Assign { local, value } => {
                let value = self.lower_expr(value)?;
                self.push(lir::Instruction::Store {
                    local: self.local_slot(*local),
                    value,
                });
            }
            mir::StatementKind::GlobalAssign { global, value } => {
                let ty = &value.ty;
                let value = self.lower_expr(value)?;
                match *self
                    .storage_globals
                    .get(global)
                    .expect("every MIR global has storage")
                {
                    StorageGlobal::Local(global) => {
                        self.push(lir::Instruction::GlobalStore { global, value })
                    }
                    StorageGlobal::Native(global) => {
                        let value = self.project_c_value(ty, value);
                        let safepoint =
                            self.new_safepoint(lir::SafepointSiteRole::NativeSafeTransition);
                        self.push(lir::Instruction::NativeGlobalStore {
                            global,
                            value,
                            safepoint,
                            roots: lir::NativeSafeRootSet::default(),
                        })
                    }
                }
            }
            // `m[i] = v`: bounds check and the element store are
            // codegen's job; the element layout comes from the array
            // operand's type.
            mir::StatementKind::ArraySet {
                array_type,
                array,
                index,
                value,
            } => {
                let array = self.lower_expr(array)?;
                let index = self.lower_expr(index)?;
                let value = self.lower_expr(value)?;
                self.push(lir::Instruction::ArraySet {
                    array,
                    index,
                    value,
                    array_type: self.array_type_id(*array_type),
                });
            }
            // `obj.field = value`: MIR carries the flattened field
            // index; LIR fixes it to the class layout's byte offset.
            mir::StatementKind::FieldSet {
                object,
                index,
                value,
            } => {
                let object_ty = object.ty.clone();
                let mir::Type::Class(class_id) = &object_ty else {
                    unreachable!("a field store targets a class object")
                };
                let field_ty =
                    &self.module.classes[*class_id].declared_fields()[*index as usize].ty;
                assert_eq!(
                    &value.ty, field_ty,
                    "a field store value must match its declared field type"
                );
                let (offsets, _, _) = class_shape(
                    self.context,
                    self.module,
                    self.enums,
                    &self.module.classes[*class_id],
                )?;
                let offset = offsets[*index as usize];
                let object = self.lower_expr(object)?;
                let value = self.lower_expr(value)?;
                self.store_heap_value(object, offset, value, field_ty)?;
            }
            mir::StatementKind::AtomicFieldStore {
                kind,
                object,
                index,
                value,
            } => {
                let object_ty = object.ty.clone();
                let mir::Type::Class(class_id) = &object_ty else {
                    unreachable!("an atomic field store targets a class object")
                };
                assert!(kind.is_atomic_state(), "only coroutine state is atomic");
                assert_eq!(
                    self.module.classes[*class_id].declared_fields()[*index as usize].ty,
                    mir::Type::MachineScalar(*kind),
                    "an atomic state operation must match its field domain"
                );
                assert_eq!(
                    value.ty,
                    mir::Type::MachineScalar(*kind),
                    "an atomic store value must match its field domain"
                );
                let (offsets, _, _) = class_shape(
                    self.context,
                    self.module,
                    self.enums,
                    &self.module.classes[*class_id],
                )?;
                let object = self.lower_expr(object)?;
                let value = self.lower_expr(value)?;
                let kind = machine_scalar_kind(*kind);
                self.push(lir::Instruction::AtomicStore {
                    kind,
                    object,
                    offset: offsets[*index as usize],
                    value,
                });
            }
            mir::StatementKind::Eh(eh) => self.lower_eh_statement(eh)?,
        }
        Ok(())
    }

    fn lower_eh_statement(&mut self, statement: &mir::EhStatement) -> StorageResult<()> {
        match statement {
            mir::EhStatement::LandingPad { cleanup } => {
                let (record_slot, raw_slot) = self.exception_slots()?;
                let record = self.new_temp(lir::LirType::ExceptionRecord);
                let raw = self.new_temp(lir::RAW_PTR);
                if *cleanup {
                    self.push(lir::Instruction::CleanupPad { record, raw });
                } else {
                    self.push(lir::Instruction::LandingPad { record, raw });
                }
                self.push(lir::Instruction::Store {
                    local: record_slot,
                    value: lir::Value::Temp(record),
                });
                self.push(lir::Instruction::Store {
                    local: raw_slot,
                    value: lir::Value::Temp(raw),
                });
            }
            mir::EhStatement::BeginCatch => {
                let (_, raw_slot) = self.exception_slots()?;
                let exception = self.new_temp(lir::MANAGED_PTR);
                self.push(lir::Instruction::BeginCatch {
                    out: exception,
                    raw: lir::Value::Local(raw_slot),
                });
                let slot = match self.caught_exception {
                    Some(slot) => slot,
                    None => {
                        let slot = self.new_hidden_local(lir::MANAGED_PTR)?;
                        self.caught_exception = Some(slot);
                        slot
                    }
                };
                self.push(lir::Instruction::Store {
                    local: slot,
                    value: lir::Value::Temp(exception),
                });
            }
            mir::EhStatement::EndCatch => self.push(lir::Instruction::EndCatch),
        }
        Ok(())
    }

    pub(super) fn lower_terminator(&mut self, terminator: &mir::Terminator) -> StorageResult<()> {
        match terminator {
            mir::Terminator::Goto(target) => {
                self.seal(lir::Terminator::Br(self.block_map[target]));
            }
            mir::Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                let cond = self.lower_expr(cond)?;
                self.seal(lir::Terminator::CondBr {
                    cond,
                    then_block: self.block_map[then_block],
                    else_block: self.block_map[else_block],
                });
            }
            mir::Terminator::Return { value } => {
                let value = match (self.returns_void, value) {
                    (true, None) => None,
                    (true, Some(value)) => {
                        self.lower_expr(value)?;
                        None
                    }
                    (false, Some(value)) => Some(self.lower_expr(value)?),
                    (false, None) => unreachable!("non-Unit return without a value"),
                };
                self.seal(lir::Terminator::Return { value });
            }
            mir::Terminator::Throw { exception, unwind } => {
                let value = self.lower_expr(exception)?;
                if let Some(unwind) = unwind {
                    let normal = self.new_block("throw.normal");
                    let (call, _) =
                        self.typed_call(vec![lir::MANAGED_PTR], lir::LirType::Void, vec![value])?;
                    let site = self.invoke_site(
                        LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::Throw),
                        call,
                        normal,
                        self.block_map[unwind],
                    );
                    self.push(lir::Instruction::Invoke { site });
                    self.seal(lir::Terminator::Br(normal));
                    self.enter(normal);
                    self.seal(lir::Terminator::Unreachable);
                } else {
                    self.push(lir::Instruction::Throw { exception: value });
                    self.seal(lir::Terminator::Unreachable);
                }
            }
            mir::Terminator::Rethrow { unwind } => match unwind {
                Some(unwind) => {
                    let normal = self.new_block("rethrow.normal");
                    let (call, _) = self.typed_call(Vec::new(), lir::LirType::Void, Vec::new())?;
                    let site = self.invoke_site(
                        LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::Rethrow),
                        call,
                        normal,
                        self.block_map[unwind],
                    );
                    self.push(lir::Instruction::Invoke { site });
                    self.seal(lir::Terminator::Br(normal));
                    self.enter(normal);
                    self.seal(lir::Terminator::Unreachable);
                }
                None => {
                    self.emit_plain_call(
                        LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::Rethrow),
                        Vec::new(),
                        lir::LirType::Void,
                        Vec::new(),
                    )?;
                    self.seal(lir::Terminator::Unreachable);
                }
            },
            mir::Terminator::Resume => {
                let (record, _) = self.exception_slots()?;
                self.seal(lir::Terminator::Resume {
                    exception: lir::Value::Local(record),
                });
            }
            mir::Terminator::Trap { message } => {
                let trap = self.trap_block(message)?;
                self.seal(lir::Terminator::Br(trap));
            }
            mir::Terminator::Unreachable => self.seal(lir::Terminator::Unreachable),
        }
        Ok(())
    }
}
