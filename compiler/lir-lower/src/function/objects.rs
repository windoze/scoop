use super::*;

mod storage;

impl FunctionLowerer<'_> {
    pub(super) fn lower_class_alloc(
        &mut self,
        class_id: &mir::ClassId,
    ) -> StorageResult<lir::Value> {
        let def = &self.module.classes[*class_id];
        let (_, size, _) = class_shape(self.context, self.module, self.enums, def)?;
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

    pub(super) fn lower_closure_alloc(
        &mut self,
        class: &mir::ClosureClassId,
        captures: &[mir::ClosureCaptureInit],
    ) -> StorageResult<lir::Value> {
        let def = &self.module.closure_classes[*class];
        let (capture_offsets, size, _, _) =
            closure_shape(self.context, self.module, self.enums, def)?;
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
        )?;
        let invoke_function = self.module.closure_invoke_functions[def.invoke].function;
        let invoke = self.new_temp(lir::CODE_PTR);
        self.push(lir::Instruction::FunctionAddress {
            out: invoke,
            target: lir::FunctionAddressTarget::Local(self.local_function_map[&invoke_function]),
        });
        let (invoke_offset, _) = self.context.closure_prefix();
        self.store_at_offset(
            object,
            invoke_offset,
            lir::Value::Temp(invoke),
            lir::CODE_PTR,
        );
        let mut initialized = vec![false; def.captures.len()];
        for capture in captures {
            let field = usize::try_from(capture.field())
                .expect("a closure field index fits the target address space");
            let capture_ty = &def
                .captures
                .get(field)
                .expect("ClosureAlloc names an existing physical field")
                .ty;
            assert_eq!(
                &capture.value().ty,
                capture_ty,
                "ClosureAlloc capture type matches its storage field"
            );
            assert!(
                !std::mem::replace(&mut initialized[field], true),
                "ClosureAlloc initializes each physical field once"
            );
            let value = self.lower_expr(capture.value())?;
            self.store_heap_value(object, capture_offsets[field], value, capture_ty)?;
        }
        assert!(
            initialized.into_iter().all(|initialized| initialized),
            "ClosureAlloc initializes every physical field"
        );
        Ok(object)
    }

    pub(super) fn lower_closure_capture(
        &mut self,
        ty: &mir::Type,
        closure: &mir::Expr,
        class: &mir::ClosureClassId,
        index: &u32,
    ) -> StorageResult<lir::Value> {
        let def = &self.module.closure_classes[*class];
        let capture_ty = &def.captures[*index as usize].ty;
        assert_eq!(
            ty, capture_ty,
            "a closure capture read has its declared storage type"
        );
        let (capture_offsets, _, _, _) = closure_shape(self.context, self.module, self.enums, def)?;
        let closure = self.lower_expr(closure)?;
        self.load_heap_value(closure, capture_offsets[*index as usize], ty)
    }

    pub(super) fn lower_field_access(
        &mut self,
        ty: &mir::Type,
        receiver: &mir::Expr,
        index: &u32,
    ) -> StorageResult<lir::Value> {
        let receiver_ty = receiver.ty.clone();
        if let mir::Type::Class(class_id) = &receiver_ty {
            let field_ty = &self.module.classes[*class_id].declared_fields()[*index as usize].ty;
            assert_eq!(ty, field_ty, "a field access has its declared field type");
        }
        let receiver = self.lower_expr(receiver)?;
        if let mir::Type::Class(class_id) = receiver_ty {
            let (offsets, _, _) = class_shape(
                self.context,
                self.module,
                self.enums,
                &self.module.classes[class_id],
            )?;
            self.load_heap_value(receiver, offsets[*index as usize], ty)
        } else {
            let c_layout = matches!(receiver_ty, mir::Type::Struct(id)
                if self.structs[struct_def_id(id)].is_c_layout());
            let out_ty = if c_layout {
                self.c_storage_type(ty)
            } else {
                self.value_type(ty)
            };
            let out = self.new_temp(out_ty);
            self.push(lir::Instruction::ExtractValue {
                out,
                aggregate: receiver,
                index: *index,
            });
            let value = lir::Value::Temp(out);
            Ok(if c_layout {
                self.restore_c_value(ty, value)
            } else {
                value
            })
        }
    }

    pub(super) fn lower_atomic_field_load(
        &mut self,
        kind: &mir::MachineScalarKind,
        object: &mir::Expr,
        index: &u32,
    ) -> StorageResult<lir::Value> {
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
        )?;
        let object = self.lower_expr(object)?;
        let kind = machine_scalar_kind(*kind);
        let out = self.new_temp(lir::LirType::MachineScalar(kind));
        self.push(lir::Instruction::AtomicLoad {
            out,
            kind,
            object,
            offset: offsets[*index as usize],
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_atomic_field_compare_exchange(
        &mut self,
        kind: &mir::MachineScalarKind,
        object: &mir::Expr,
        index: &u32,
        expected: &mir::Expr,
        replacement: &mir::Expr,
    ) -> StorageResult<lir::Value> {
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
        )?;
        let object = self.lower_expr(object)?;
        let expected = self.lower_expr(expected)?;
        let replacement = self.lower_expr(replacement)?;
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
        Ok(lir::Value::Temp(out))
    }
}
