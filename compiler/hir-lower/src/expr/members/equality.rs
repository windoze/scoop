use super::*;

pub(crate) struct ImportedDerivedEqualityProbe {
    pub(crate) target: hir::ImportedDerivedEquality,
    pub(crate) owner: TypeId,
    layer: SuccessfulExprLayer,
}

impl ImportedDerivedEqualityProbe {
    pub(in crate::expr) fn commit(
        self,
        state: &mut Lowerer,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        state.commit_expr_layer(self.layer, sink)
    }
}

impl Lowerer {
    pub(super) fn probe_imported_derived_equality(
        &self,
        target: hir::ImportedDerivedEquality,
        receiver: hir::Expr,
        call: CallSite<'_>,
        expected: Option<TypeId>,
    ) -> Result<ImportedDerivedEqualityProbe, Box<Lowerer>> {
        let mut state = self.clone();
        let owner = receiver.ty;
        let [argument] = call.args else {
            state.error(
                call.span,
                format!(
                    "derived member `equals` requires one argument, found {}",
                    call.args.len()
                ),
            );
            return Err(Box::new(state));
        };
        if !call.type_args.is_empty() {
            state.error(
                call.span,
                "derived member `equals` does not accept type arguments".into(),
            );
            return Err(Box::new(state));
        }
        if let ast::CallArgumentName::Named(name) = &argument.name
            && name.text != "other"
        {
            state.error(
                name.span,
                format!("derived member `equals` has no parameter `{}`", name.text),
            );
            return Err(Box::new(state));
        }
        if let ast::SpreadSyntax::Spread(span) = argument.spread {
            state.error(
                span,
                "derived member `equals` does not have a vararg parameter".into(),
            );
            return Err(Box::new(state));
        }
        if let Some(expected) = expected
            && !state.is_subtype(state.boolean, expected)
        {
            state.error(
                call.span,
                format!(
                    "derived member `equals` returns Boolean, expected {}",
                    state.type_name(expected)
                ),
            );
            return Err(Box::new(state));
        }
        let mut sink = Vec::new();
        let Some((receiver, other)) = state.lower_ordered_rhs(
            receiver,
            &argument.expression,
            &mut sink,
            Some(owner),
            "$equality.lhs",
        ) else {
            return Err(Box::new(state));
        };
        if !state.is_subtype(other.ty, owner) {
            state.error(
                argument.span,
                format!(
                    "argument `other` for derived member `equals` must have type {}, found {}",
                    state.type_name(owner),
                    state.type_name(other.ty),
                ),
            );
            return Err(Box::new(state));
        }
        let other = state.adapt_to(other, owner);
        if state.requires_unsafe_use(owner) {
            state.require_unsafe_operation(call.span, "calling an unsafe dependency function");
        }
        let expression = state.imported_equality_call(target, receiver, other, call.span);
        Ok(ImportedDerivedEqualityProbe {
            target,
            owner,
            layer: SuccessfulExprLayer {
                state: Box::new(state),
                expression,
                sink,
            },
        })
    }
}
