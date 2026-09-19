use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_box(&mut self, operand: &mir::Expr) -> lir::Value {
        let value = self.lower_expr(operand);
        let descriptor = self.type_descriptors.for_boxed_type(&operand.ty);
        let payload = match descriptor {
            lir::BoxedValueDescriptor::ZeroSized(descriptor) => {
                lir::BoxPayload::ZeroSized(descriptor)
            }
            lir::BoxedValueDescriptor::NonZero(descriptor) => {
                let local = self.box_value_local(descriptor.value().clone());
                self.push(lir::Instruction::Store { local, value });
                lir::BoxPayload::NonZero(
                    descriptor
                        .bind_place(&self.locals, local)
                        .expect("fresh box source has the descriptor's complete value layout"),
                )
            }
        };
        let out = self.new_temp(lir::MANAGED_PTR);
        let safepoint = self.new_safepoint(lir::SafepointSiteRole::ManagedCall);
        self.push(lir::Instruction::BoxValue {
            out,
            payload,
            safepoint,
            live: lir::StatepointLiveSet::default(),
        });
        lir::Value::Temp(out)
    }

    pub(super) fn lower_unbox(&mut self, operand: &mir::Expr, ty: &mir::Type) -> lir::Value {
        let object = self.lower_expr(operand);
        let descriptor = self.type_descriptors.for_boxed_type(ty);
        let (result, value) = match descriptor {
            lir::BoxedValueDescriptor::ZeroSized(descriptor) => {
                let out = self.new_temp(descriptor.value().representation().storage_type().clone());
                (
                    lir::UnboxResult::ZeroSized { descriptor, out },
                    lir::Value::Temp(out),
                )
            }
            lir::BoxedValueDescriptor::NonZero(descriptor) => {
                let local = self.box_value_local(descriptor.value().clone());
                let place = descriptor
                    .bind_place(&self.locals, local)
                    .expect("fresh unbox destination has the descriptor's complete value layout");
                (lir::UnboxResult::NonZero(place), lir::Value::Local(local))
            }
        };
        self.push(lir::Instruction::UnboxValue { object, result });
        value
    }

    fn box_value_local(&mut self, value: lir::AbiValue) -> lir::LocalId {
        self.hidden_count += 1;
        self.locals.alloc(lir::Local::new(
            format!("$box.{}", self.hidden_count),
            lir::LocalStorage::NonZero(value),
        ))
    }
}
