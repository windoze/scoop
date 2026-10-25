use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_string_concat(
        &mut self,
        parts: &[mir::StringConcatPart],
    ) -> StorageResult<lir::Value> {
        let parts = parts
            .iter()
            .map(|part| {
                Ok(match part {
                    mir::StringConcatPart::Value(value) => {
                        lir::StringConcatPart::Value(self.lower_expr(value)?)
                    }
                    mir::StringConcatPart::Literal(text) => {
                        lir::StringConcatPart::Literal(text.clone())
                    }
                })
            })
            .collect::<StorageResult<Vec<_>>>()?;
        let out = self.new_temp(lir::MANAGED_PTR);
        let safepoint = self.new_safepoint(lir::SafepointSiteRole::ManagedCall);
        let overflow_message = self.trap_message("String size overflow");
        self.push(lir::Instruction::StringConcat {
            out,
            parts,
            overflow_message,
            descriptor: self.type_descriptors.for_type(&mir::Type::String),
            safepoint,
            live: lir::StatepointLiveSet::default(),
        });
        Ok(lir::Value::Temp(out))
    }
}
