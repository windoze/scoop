use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn project_dispatch_receiver(
        &mut self,
        kind: &mir::CallKind,
        args: &mut [lir::Value],
        signature: &mut lir::ScoopAbiSignature,
    ) -> StorageResult<()> {
        if matches!(kind, mir::CallKind::Interface { .. }) {
            args[0] = self.interface_component(args[0], 0);
            let mut parameters = signature
                .arguments()
                .iter()
                .map(|argument| argument.logical_storage_type().clone())
                .collect::<Vec<_>>();
            parameters[0] = lir::MANAGED_PTR;
            *signature = abi::classify_signature(
                self.context,
                parameters,
                signature.result().logical_storage_type().cloned(),
                self.structs,
                self.enums,
            )?;
        }
        Ok(())
    }

    pub(super) fn interface_component(&mut self, value: lir::Value, index: u32) -> lir::Value {
        let out = self.new_temp(if index == 0 {
            lir::MANAGED_PTR
        } else {
            lir::METADATA_PTR
        });
        self.push(lir::Instruction::ExtractValue {
            out,
            aggregate: value,
            index,
        });
        lir::Value::Temp(out)
    }

    pub(super) fn interface_kind(&self, ty: &mir::Type) -> Option<mir::InterfaceId> {
        match ty {
            mir::Type::Interface(id) => Some(*id),
            mir::Type::Enum(id, _)
                if matches!(
                    self.enums[enum_def_id(*id)].repr,
                    lir::EnumRepr::Niche {
                        kind: lir::NullNicheKind::Interface,
                        ..
                    }
                ) =>
            {
                self.module.enums[*id]
                    .variants
                    .iter()
                    .flat_map(|variant| &variant.fields)
                    .find_map(|field| match field.ty {
                        mir::Type::Interface(id) => Some(id),
                        _ => None,
                    })
            }
            _ => None,
        }
    }

    pub(super) fn reference_object(&mut self, value: lir::Value, ty: &mir::Type) -> lir::Value {
        if self.interface_kind(ty).is_some() {
            self.interface_component(value, 0)
        } else {
            value
        }
    }

    pub(super) fn lower_reference_conversion(
        &mut self,
        operand: &mir::Expr,
        ty: &mir::Type,
    ) -> StorageResult<lir::Value> {
        let value = self.lower_expr(operand)?;
        if &operand.ty == ty {
            return Ok(value);
        }
        let object = self.reference_object(value, &operand.ty);
        let Some(interface) = self.interface_kind(ty) else {
            return Ok(object);
        };
        let known_class = match (&operand.kind, &operand.ty) {
            (mir::ExprKind::ClassAlloc { class_id }, _) => Some(*class_id),
            (mir::ExprKind::Local(local), _)
                if self
                    .known_receiver
                    .is_some_and(|(receiver, _)| receiver == *local) =>
            {
                self.known_receiver.map(|(_, class)| class)
            }
            (_, mir::Type::Class(id))
                if self.module.classes[*id].modifier == mir::ClassModifier::Final =>
            {
                Some(*id)
            }
            _ => None,
        };
        let table = self.interface_table(object, interface, known_class)?;
        Ok(self.make_aggregate(ty, vec![object, table]))
    }

    pub(super) fn interface_table(
        &mut self,
        object: lir::Value,
        interface: mir::InterfaceId,
        known_class: Option<mir::ClassId>,
    ) -> StorageResult<lir::Value> {
        if self.module.interfaces[interface].methods.is_empty() {
            return Ok(lir::Value::NullPointer(lir::PointerKind::Metadata));
        }
        if let Some(class) = known_class {
            let index = self.type_descriptors.interface_table_index(
                class,
                exact_type_record(self.module, &mir::Type::Interface(interface)).id(),
            );
            let descriptor = self.td_ref(&self.module.classes[class].physical_type(class));
            let directory = self.load_at_offset(
                descriptor,
                self.context.type_descriptor_itables_offset(),
                lir::METADATA_PTR,
            );
            let table = self.load_at_offset(
                lir::Value::Temp(directory),
                index as u64 * 16 + 8,
                lir::METADATA_PTR,
            );
            return Ok(lir::Value::Temp(table));
        }
        let descriptor = self.load_at_offset(
            object,
            self.context.object_type_descriptor_offset(),
            lir::METADATA_PTR,
        );
        self.emit_plain_call(
            LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::ITableLookup),
            vec![lir::METADATA_PTR, lir::METADATA_PTR],
            lir::METADATA_PTR,
            vec![
                lir::Value::Temp(descriptor),
                self.td_ref(&mir::Type::Interface(interface)),
            ],
        )
    }

    pub(super) fn restore_reference_value(
        &mut self,
        object: lir::Value,
        ty: &mir::Type,
    ) -> StorageResult<lir::Value> {
        let Some(interface) = self.interface_kind(ty) else {
            return Ok(object);
        };
        if matches!(ty, mir::Type::Interface(_)) {
            let table = self.interface_table(object, interface, None)?;
            return Ok(self.make_aggregate(ty, vec![object, table]));
        }
        let storage_type = self.value_type(ty);
        let storage = self.new_hidden_local(storage_type)?;
        let empty = self.make_aggregate(
            ty,
            vec![
                lir::Value::NullPointer(lir::PointerKind::Managed),
                lir::Value::NullPointer(lir::PointerKind::Metadata),
            ],
        );
        self.push(lir::Instruction::Store {
            local: storage,
            value: empty,
        });
        let non_null = self.new_temp(lir::LirType::I1);
        self.push(lir::Instruction::BinOp {
            out: non_null,
            op: lir::BinOp::Ne,
            lhs: object,
            rhs: lir::Value::NullPointer(lir::PointerKind::Managed),
        });
        let present = self.new_block("interface_present");
        let done = self.new_block("interface_ready");
        self.seal(lir::Terminator::CondBr {
            cond: lir::Value::Temp(non_null),
            then_block: present,
            else_block: done,
        });
        self.enter(present);
        let table = self.interface_table(object, interface, None)?;
        let value = self.make_aggregate(ty, vec![object, table]);
        self.push(lir::Instruction::Store {
            local: storage,
            value,
        });
        self.seal(lir::Terminator::Br(done));
        self.enter(done);
        Ok(lir::Value::Local(storage))
    }
}
