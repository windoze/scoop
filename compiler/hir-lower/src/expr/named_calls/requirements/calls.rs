use super::*;
use crate::call_resolution::specificity::{ApplicableDeclaration, DeclarationForwardingView};

impl Lowerer {
    pub(in crate::expr) fn lower_required_member_call(
        &mut self,
        receiver: Option<hir::Expr>,
        subject: TypeId,
        name: Option<&str>,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<Option<hir::Expr>> {
        let requirements = self
            .type_params_in_scope
            .iter()
            .flat_map(|parameter| &parameter.requirements)
            .filter(|requirement| requirement.contract.subject == subject)
            .filter(|requirement| match (&requirement.contract.call, name) {
                (hir::RequiredCallKind::Constructor, None) => true,
                (hir::RequiredCallKind::Method { name: member, .. }, Some(name)) => member == name,
                _ => false,
            })
            .cloned()
            .collect::<Vec<_>>();
        if requirements.is_empty() {
            return None;
        }
        let mut candidates = Vec::new();
        for requirement in requirements {
            if let Ok(layer) = self.probe_expr_layer(|state, sink| {
                state.lower_requirement_invocation(&requirement, receiver.clone(), call, sink)
            }) {
                candidates.push((requirement, layer));
            }
        }
        if candidates.is_empty() {
            return None;
        }
        let types = candidates
            .iter()
            .map(|(requirement, _)| {
                requirement
                    .contract
                    .arguments
                    .iter()
                    .map(|argument| argument.ty)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let pool = types
            .iter()
            .map(|types| ApplicableDeclaration {
                declaration: DeclarationForwardingView::parameter_groups(&[], &[], types),
                parameterized: false,
                defaults: 0,
                vararg: false,
            })
            .collect::<Vec<_>>();
        let winners = self.most_specific_declarations(&pool);
        if winners.len() != 1 {
            self.error(call.span, "ambiguous member requirement invocation".into());
            return Some(None);
        }
        let (requirement, layer) = candidates.swap_remove(winners[0]);
        let expression = self.commit_expr_layer(layer, sink);
        let effects = requirement.contract.effects;
        if effects.is_suspend {
            self.check_suspend_context(name.unwrap_or("constructor"), call.span);
        }
        if effects.safety == hir::Safety::Unsafe {
            self.require_unsafe_operation(call.span, "unsafe member requirement invocation");
        }
        Some(Some(expression))
    }

    fn lower_requirement_invocation(
        &mut self,
        requirement: &hir::MemberRequirement,
        receiver: Option<hir::Expr>,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let contract = &requirement.contract;
        if call.type_args.len() != contract.type_arguments().len()
            || call.args.len() != contract.arguments.len()
        {
            return None;
        }
        for (actual, required) in call.type_args.iter().zip(contract.type_arguments()) {
            match (actual, required) {
                (ast::CallTypeArgument::Infer { .. }, hir::RequiredTypeArgument::Infer) => {}
                (
                    ast::CallTypeArgument::Explicit(actual),
                    hir::RequiredTypeArgument::Explicit(required),
                ) if self.resolve_type_ref(actual) == Some(*required) => {}
                _ => return None,
            }
        }
        let mut arguments = Vec::new();
        if let Some(receiver) = receiver {
            arguments.push(self.materialize_temporary(
                "requirement.receiver".into(),
                receiver,
                call.span,
                sink,
            ));
        }
        for (actual, required) in call.args.iter().zip(&contract.arguments) {
            let label = match &actual.name {
                ast::CallArgumentName::Named(name) => Some(name.text.as_str()),
                ast::CallArgumentName::Positional | ast::CallArgumentName::TrailingLambda => None,
            };
            if label != required.name.as_deref()
                || matches!(actual.spread, ast::SpreadSyntax::Spread(_)) != required.spread
            {
                return None;
            }
            let value = self.lower_expr(&actual.expression, sink, Some(required.ty))?;
            if !self.is_subtype(value.ty, required.ty) {
                return None;
            }
            let value = self.adapt_to(value, required.ty);
            arguments.push(self.materialize_temporary(
                "requirement.argument".into(),
                value,
                actual.span,
                sink,
            ));
        }
        let result = self.alloc_hidden("requirement.result", contract.result);
        let origin = self.expression_origin(call.span);
        sink.push(hir::Statement {
            kind: hir::StatementKind::MemberRequirementCall(Box::new(hir::MemberRequirementCall {
                contract: contract.clone(),
                binding: Binding::Forwarded(requirement.id),
                arguments,
                result,
                origin,
            })),
            span: call.span,
        });
        Some(hir::Expr {
            kind: ExprKind::Local(result),
            ty: contract.result,
            span: call.span,
            origin,
        })
    }
}
