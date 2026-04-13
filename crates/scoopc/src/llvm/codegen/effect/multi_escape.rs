impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn build_multiple_escape_binder_slots<'hir>(
        &mut self,
        arm: &'hir hir::HandleArm,
        name_prefix: &str,
    ) -> Result<Vec<ImmediateResumeBinderSlot<'ctx>>, LlvmEmitError> {
        let mut slots = Vec::with_capacity(arm.op.binders.len());
        for (idx, binder) in arm.op.binders.iter().enumerate() {
            let binder_ty =
                self.cg_ty_of(binder.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multiple escape-continuation arms binder type",
                        at: binder.span.into(),
                    })?;
            let slot_name = format!("{name_prefix}_{idx}_{}", binder.name);
            let ptr = self.create_entry_alloca(binder.span, &slot_name, binder_ty)?;
            slots.push(ImmediateResumeBinderSlot {
                id: binder.id,
                hir_ty: binder.ty,
                ty: binder_ty,
                ptr,
            });
        }
        Ok(slots)
    }

    fn codegen_handle_expr_multiple_escape_top_level_direct<'hir>(
        &mut self,
        _span: crate::span::Span,
        _handle: &'hir hir::HandleExpr,
        _state_machine_plan: &HandleStateMachinePlan,
        _escape_arms: &[(&'hir hir::HandleArm, hir::SymbolId)],
        _sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        _out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        unimplemented!("legacy mixed.rs/matrix.rs 已删除；需改走 unified emitter")
    }
}
