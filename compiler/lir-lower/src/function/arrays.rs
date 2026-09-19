use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_array_literal(
        &mut self,
        ty: &mir::Type,
        array_type: &mir::ClassId,
        elements: &[mir::Expr],
    ) -> StorageResult<lir::Value> {
        let elements: Vec<lir::Value> = elements
            .iter()
            .map(|element| self.lower_expr(element))
            .collect::<StorageResult<Vec<_>>>()?;
        let out_ty = self.value_type(ty);
        let out = self.new_temp(out_ty);
        let safepoint = self.new_safepoint(lir::SafepointSiteRole::ManagedCall);
        self.push(lir::Instruction::ArrayAlloc {
            out,
            elements,
            array_type: self.array_type_id(*array_type),
            safepoint,
            live: lir::StatepointLiveSet::default(),
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_array_assembly(
        &mut self,
        ty: &mir::Type,
        array_type: &mir::ClassId,
        parts: &[mir::ArrayAssemblyPart],
    ) -> StorageResult<lir::Value> {
        let parts = parts
            .iter()
            .map(|part| {
                Ok(match part {
                    mir::ArrayAssemblyPart::Element(value) => {
                        lir::ArrayAssemblyPart::Element(self.lower_expr(value)?)
                    }
                    mir::ArrayAssemblyPart::CopyArray(value) => {
                        lir::ArrayAssemblyPart::CopyArray(self.lower_expr(value)?)
                    }
                })
            })
            .collect::<StorageResult<Vec<_>>>()?;
        let out_ty = self.value_type(ty);
        let out = self.new_temp(out_ty);
        let safepoint = self.new_safepoint(lir::SafepointSiteRole::ManagedCall);
        self.push(lir::Instruction::ArrayAssembly {
            out,
            parts,
            array_type: self.array_type_id(*array_type),
            safepoint,
            live: lir::StatepointLiveSet::default(),
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_array_get(
        &mut self,
        ty: &mir::Type,
        array_type: &mir::ClassId,
        array: &mir::Expr,
        index: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        let array = self.lower_expr(array)?;
        let index = self.lower_expr(index)?;
        let out_ty = self.value_type(ty);
        let out = self.new_temp(out_ty);
        self.push(lir::Instruction::ArrayGet {
            out,
            array,
            index,
            array_type: self.array_type_id(*array_type),
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_array_len(
        &mut self,
        ty: &mir::Type,
        array_type: &mir::ClassId,
        operand: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        assert_eq!(
            *ty,
            mir::Type::Integer(mir::IntegerKind::SIGNED_64),
            "array length is canonical Long",
        );
        let operand = self.lower_expr(operand)?;
        let out = self.new_temp(lir::LirType::I64);
        self.push(lir::Instruction::ArrayLen {
            out,
            operand,
            array_type: self.array_type_id(*array_type),
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_array_clone(
        &mut self,
        ty: &mir::Type,
        source_type: &mir::ClassId,
        target_type: &mir::ClassId,
        operand: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        let operand = self.lower_expr(operand)?;
        let out_ty = self.value_type(ty);
        let out = self.new_temp(out_ty);
        let safepoint = self.new_safepoint(lir::SafepointSiteRole::ManagedCall);
        self.push(lir::Instruction::ArrayClone {
            out,
            operand,
            source_type: self.array_type_id(*source_type),
            array_type: self.array_type_id(*target_type),
            safepoint,
            live: lir::StatepointLiveSet::default(),
        });
        Ok(lir::Value::Temp(out))
    }
}
