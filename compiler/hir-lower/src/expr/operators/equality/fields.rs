use super::*;
use crate::call_resolution::named::NamedFunctionLikeProbe;
use crate::overload::{LoweredOverloadCall, NamedCallReceiver};

impl Lowerer {
    pub(crate) fn resolve_derived_field_member_equality(
        &mut self,
        lhs: hir::Expr,
        rhs: hir::Expr,
        path: &str,
        span: Span,
    ) -> Result<hir::Expr, String> {
        let mut state = self.clone();
        let name = ast::Ident {
            text: "equals".into(),
            span,
        };
        let missing = || {
            format!(
                "field `{path}` of type `{}` has no applicable member operator `equals`",
                self.type_name(lhs.ty)
            )
        };
        let local = state.methods_by_operator(lhs.ty, hir::OperatorKind::Equals);
        let imported = state
            .imported_member_call_candidates(
                lhs.ty,
                &name,
                RequiredCallableModifiers {
                    operator: Some(hir::OperatorKind::Equals),
                    ..Default::default()
                },
                MemberCallKind::Ordinary,
            )
            .map_err(|_| missing())?;
        let mut probes = Vec::new();
        for candidate in local {
            if state
                .declaration_surface
                .rejects_function(candidate.function)
            {
                continue;
            }
            if let Ok(probe) = state.probe_lowered_named_callable(
                "equals",
                candidate,
                NamedCallReceiver::Member(lhs.clone()),
                LoweredOverloadCall {
                    explicit_type_args: Vec::new(),
                    args: vec![rhs.clone()],
                    span,
                    expected_result: Some(state.boolean),
                },
            ) {
                probes.push(NamedFunctionLikeProbe::Callable(Box::new(probe)));
            }
        }
        for candidate in imported {
            if let Ok(probe) = state.probe_imported_member_callable(
                candidate,
                ImportedMemberReceiver::Value(lhs.clone()),
                &name,
                ImportedProbeCall::lowered(std::slice::from_ref(&rhs), span),
                Some(state.boolean),
                false,
            ) {
                probes.push(NamedFunctionLikeProbe::ImportedDependency(Box::new(probe)));
            }
        }
        if probes.is_empty() {
            return Err(missing());
        }
        let winner = state
            .select_lowered_named_function_like("equals", "member", &probes, span)
            .ok_or_else(|| {
                format!(
                    "field `{path}` has an ambiguous member operator `equals` for type `{}`",
                    self.type_name(lhs.ty)
                )
            })?;
        let mut sink = Vec::new();
        let expression = match probes.swap_remove(winner) {
            NamedFunctionLikeProbe::Callable(probe) => state
                .commit_named_callable(*probe, &mut sink)
                .and_then(|resolved| state.finish_resolved_method_call(resolved, span, &mut sink)),
            NamedFunctionLikeProbe::ImportedDependency(probe) => {
                state.commit_imported_lowered_callable(*probe)
            }
            _ => unreachable!("equality candidates are declared members"),
        }
        .ok_or_else(missing)?;
        assert!(
            sink.is_empty(),
            "derived equality reads already prepared fields"
        );
        *self = state;
        Ok(expression)
    }
}
