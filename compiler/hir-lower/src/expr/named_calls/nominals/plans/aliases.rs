use super::*;

impl Lowerer {
    pub(in crate::expr) fn named_alias_plans(
        &mut self,
        mut alias: crate::aliases::AliasTemplate,
        call: &ast::CallExpr,
        expected: Option<TypeId>,
    ) -> Result<PreparedNominalPlans, ()> {
        let generic = !alias.parameters.is_empty();
        if !generic && !call.type_args.is_empty() {
            self.error(
                call.callee.span,
                format!("typealias `{}` is not generic", call.callee.text),
            );
            return Err(());
        }
        let expected = if generic {
            expected
        } else {
            Some(alias.target)
        };
        let target = if matches!(self.types[alias.target], Type::Param(_)) {
            let explicit = self.resolve_call_type_args(&call.type_args).ok_or(())?;
            let (target, arguments) = self
                .infer_alias_target(&alias, &explicit, expected, call.span)
                .ok_or(())?;
            alias.arguments = Some(arguments);
            target
        } else {
            alias.target
        };
        let mut plans = if let Some(nominal) = self.nominal_target_for_type(target) {
            self.nominal_constructor_plans(nominal, call, expected, false)?
        } else if let Some(owner) = self.imported_nominal_owner(target) {
            self.imported_nominal_plans(owner, call, expected, false)
        } else {
            self.error(
                call.callee.span,
                format!(
                    "typealias `{}` does not name a constructible type",
                    call.callee.text
                ),
            );
            return Err(());
        };
        match &mut plans {
            PreparedNominalPlans::Nominal(plans, pointer) => {
                for plan in plans {
                    plan.alias = Some(alias.clone());
                }
                if let Some(pointer) = pointer {
                    pointer.3 = Some(alias);
                }
            }
            PreparedNominalPlans::Imported { alias: target, .. } => *target = Some(alias),
        }
        Ok(plans)
    }
}

impl Lowerer {
    pub(in crate::expr) fn lower_alias_construct(
        &mut self,
        alias: crate::aliases::AliasTemplate,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let call = ast::CallExpr {
            callee: name.clone(),
            type_args: call.type_args.to_vec(),
            args: call.args.to_vec(),
            span: call.span,
        };
        let prepared = self.named_alias_plans(alias, &call, expected).ok()?;
        let explicit = self.resolve_call_type_args(&call.type_args)?;
        let mut applicable = Vec::new();
        let mut failures = Vec::new();
        self.collect_prepared_nominal_probes(
            prepared,
            &call,
            &explicit,
            &mut applicable,
            &mut failures,
        );
        if applicable.is_empty() {
            for failure in failures {
                self.commit_layer_diagnostics(*failure);
            }
            return None;
        }
        self.commit_named_function_partition(applicable, &[], ImportLookupLayer::Exact, &call, sink)
            .ok()
            .flatten()
    }
}
