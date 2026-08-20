use super::places::WriteCapability;
use super::*;
use crate::expr::{CallSite, RequiredCallableModifiers};

struct OperatorProbe {
    state: Box<Lowerer>,
    value: hir::Expr,
    sink: Vec<hir::Statement>,
}

struct FailedOperatorProbe {
    messages: Vec<String>,
}

impl Lowerer {
    fn probe_compound_operator(
        &self,
        receiver: hir::Expr,
        rhs: &ast::Expr,
        operator: hir::OperatorKind,
        name: &str,
        span: Span,
        expected: TypeId,
    ) -> Result<OperatorProbe, FailedOperatorProbe> {
        let mut state = self.clone();
        let diagnostic_start = state.diagnostics.len();
        let mut sink = Vec::new();
        let arguments = [ast::CallArgument::positional(rhs.clone())];
        let value = state.lower_named_call_on_receiver(
            receiver,
            &ast::Ident {
                text: name.to_string(),
                span,
            },
            CallSite {
                type_args: &[],
                args: &arguments,
                span,
            },
            &mut sink,
            Some(expected),
            RequiredCallableModifiers {
                operator: Some(operator),
                infix: false,
                ..Default::default()
            },
        );
        match value {
            Some(value) => Ok(OperatorProbe {
                state: Box::new(state),
                value,
                sink,
            }),
            None => Err(FailedOperatorProbe {
                messages: state.diagnostics[diagnostic_start..]
                    .iter()
                    .map(|diagnostic| diagnostic.message.clone())
                    .collect(),
            }),
        }
    }

    pub(super) fn lower_compound_assign(
        &mut self,
        assign: &ast::Assign,
        op: ast::CompoundAssignOp,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let mut sink = Vec::new();
        let plan = self.resolve_place_plan(&assign.target, &mut sink)?;
        let old = self.materialize_place_expr(plan.read, "old", assign.span, &mut sink);
        let (assign_operator, assign_name, fallback_operator, fallback_name) = match op {
            ast::CompoundAssignOp::Add => (
                hir::OperatorKind::PlusAssign,
                "plusAssign",
                hir::OperatorKind::Plus,
                "plus",
            ),
            ast::CompoundAssignOp::Sub => (
                hir::OperatorKind::MinusAssign,
                "minusAssign",
                hir::OperatorKind::Minus,
                "minus",
            ),
            ast::CompoundAssignOp::Mul => (
                hir::OperatorKind::TimesAssign,
                "timesAssign",
                hir::OperatorKind::Times,
                "times",
            ),
            ast::CompoundAssignOp::Div => (
                hir::OperatorKind::DivAssign,
                "divAssign",
                hir::OperatorKind::Div,
                "div",
            ),
            ast::CompoundAssignOp::Rem => (
                hir::OperatorKind::RemAssign,
                "remAssign",
                hir::OperatorKind::Rem,
                "rem",
            ),
        };
        let assign_probe = self.probe_compound_operator(
            old.clone(),
            &assign.value,
            assign_operator,
            assign_name,
            assign.span,
            self.unit,
        );
        let fallback_probe = self.probe_compound_operator(
            old,
            &assign.value,
            fallback_operator,
            fallback_name,
            assign.span,
            plan.ty,
        );
        match (assign_probe, fallback_probe) {
            (Ok(_), Ok(_)) => {
                self.error(
                    assign.span,
                    format!(
                        "compound assignment is ambiguous: both `{assign_name}` and `{fallback_name}` are applicable"
                    ),
                );
                None
            }
            (Ok(probe), Err(_)) => {
                *self = *probe.state;
                sink.extend(probe.sink);
                out.extend(sink);
                Some(hir::StatementKind::Expr(probe.value))
            }
            (Err(_), Ok(probe)) => {
                if matches!(&plan.write, WriteCapability::ReadOnly) {
                    self.error(
                        assign.span,
                        format!(
                            "operator `{fallback_name}` is applicable, but compound-assignment fallback is not writable"
                        ),
                    );
                    return None;
                }
                *self = *probe.state;
                sink.extend(probe.sink);
                if !self.is_subtype(probe.value.ty, plan.ty) {
                    self.error(
                        assign.span,
                        format!(
                            "operator `{fallback_name}` returns {}, which cannot be assigned to place of type {}",
                            self.type_name(probe.value.ty),
                            self.type_name(plan.ty)
                        ),
                    );
                    return None;
                }
                let value = self.adapt_to(probe.value, plan.ty);
                let kind = self.lower_place_write(plan.write, value, assign.span, &mut sink)?;
                out.extend(sink);
                Some(kind)
            }
            (Err(assign_failure), Err(fallback_failure)) => {
                let assign_reason = assign_failure.messages.join("; ");
                let fallback_reason = fallback_failure.messages.join("; ");
                self.error(
                    assign.span,
                    format!(
                        "compound assignment has no applicable operator:\n  - {assign_name}: {assign_reason}\n  - {fallback_name}: {fallback_reason}"
                    ),
                );
                None
            }
        }
    }
}
