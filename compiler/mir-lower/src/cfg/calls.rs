use super::*;

impl CfgLowerer<'_> {
    pub(super) fn lower_call(
        &mut self,
        call: &smir::Call,
        destination: Option<mir::LocalId>,
        span: Span,
    ) -> Option<mir::Expr> {
        let args = self.lower_operands(call.args.iter(), span)?;
        self.emit_lowered_call(
            call.target.clone(),
            args,
            call.return_ty.clone(),
            destination,
            span,
        )
    }

    pub(super) fn emit_lowered_call(
        &mut self,
        target: mir::CallTarget,
        args: Vec<mir::Expr>,
        return_ty: mir::Type,
        destination: Option<mir::LocalId>,
        span: Span,
    ) -> Option<mir::Expr> {
        let normalized = mir::Call {
            target,
            args,
            pending: self.call_pending_context(),
        };
        if return_ty == mir::Type::Unit {
            assert!(
                destination.is_none(),
                "Unit calls do not have MIR destinations"
            );
            self.push_call(mir::CallEffect::Unit(normalized), span);
            Some(mir::Expr::new(mir::Type::Unit, mir::ExprKind::UnitLiteral))
        } else {
            let destination = destination.unwrap_or_else(|| {
                self.new_hidden(
                    "call",
                    StructuralDefinitionSiteRole::SyntheticValue,
                    SyntheticLocalRole::Temporary,
                    return_ty.clone(),
                )
            });
            self.push_call(
                mir::CallEffect::Value {
                    destination,
                    call: normalized,
                },
                span,
            );
            if self.is_nothing(&return_ty) {
                self.seal(mir::Terminator::Unreachable);
                None
            } else {
                Some(mir::Expr::new(return_ty, mir::ExprKind::Local(destination)))
            }
        }
    }

    fn push_call(&mut self, effect: mir::CallEffect, span: Span) {
        self.ensure_unwind_context();
        self.call_sites.push(CallSite {
            block: self.current,
            statement: self.blocks[self.current].statements.len(),
        });
        self.push(mir::StatementKind::Call(effect), span);
    }

    pub(super) fn new_hidden(
        &mut self,
        prefix: &str,
        site_role: StructuralDefinitionSiteRole,
        role: SyntheticLocalRole,
        ty: mir::Type,
    ) -> mir::LocalId {
        self.hidden_count += 1;
        let local = self.locals.alloc(mir::Local {
            name: format!("${prefix}.{}", self.hidden_count),
            ty,
            mutable: false,
        });
        self.generated_values.push(GeneratedLocalValue {
            local,
            site_role,
            role,
        });
        local
    }
}
