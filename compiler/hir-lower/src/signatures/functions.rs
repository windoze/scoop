use super::*;

impl Lowerer {
    /// Resolve a function signature: typed parameters, value parameters
    /// and the explicit/block result. Expression results remain pending. Parameter
    /// locals are only allocated when the body is lowered (pass 3), so
    /// intrinsic functions — which have no body — keep an empty
    /// `params` list on the `hir::Function`; calls check against this
    /// resolved signature like any other function's (M7).
    pub(crate) fn resolve_signature(&mut self, id: FunctionId, decl: &ast::FunctionDecl) {
        self.resolve_function_header(id, decl.header());
        self.register_inferred_function(id, decl);
    }

    pub(crate) fn resolve_function_header(
        &mut self,
        id: FunctionId,
        decl: ast::FunctionHeaderRef<'_>,
    ) {
        // Member-only flags on a top-level function (M6).
        if decl.modifier == ast::MethodModifier::Abstract {
            self.error(
                decl.name.span,
                format!(
                    "abstract function `{}` is only allowed in abstract classes",
                    decl.name.text
                ),
            );
        }
        if decl.is_override {
            self.error(
                decl.name.span,
                format!(
                    "`{}` is marked `override` but does not override any method",
                    decl.name.text
                ),
            );
        }
        if decl.body_form == ast::FunctionBodyForm::Omitted
            && matches!(self.functions[id].kind, FunctionKind::User(_))
            && !decl.annotations.iter().any(|annotation| {
                matches!(annotation.name.single_text(), Some("Intrinsic" | "Extern"))
            })
        {
            self.error(
                decl.name.span,
                format!("function `{}` must have a body", decl.name.text),
            );
        }
        let mut type_params = Vec::new();
        for param in &decl.type_params {
            if type_params
                .iter()
                .any(|existing: &hir::TypeParamDecl| existing.name == param.name.text)
            {
                self.error(
                    param.span,
                    format!("duplicate type parameter `{}`", param.name.text),
                );
                continue;
            }
            let parameter = self.fresh_type_param(type_params.len());
            type_params.push(lower_type_param_decl(param, parameter));
        }
        type_params = self.resolve_type_parameter_constraints(
            type_params,
            0,
            &decl.type_params,
            decl.where_clause.as_ref(),
            true,
            "function",
        );
        if !type_params.is_empty() {
            self.register_generic(id, type_params.clone());
        }
        self.type_params_in_scope = type_params.clone();

        if let Some(receiver) = &decl.receiver_ty
            && let Some(receiver_ty) = self.resolve_type_ref(receiver)
        {
            self.extension_receivers.insert(id, receiver_ty);
        }

        let params = self.resolve_callable_parameters(decl.signature);
        self.functions[id].context_parameters = self.resolve_context_parameters(decl.signature);
        let return_ty = match &decl.return_ty {
            Some(ty_ref) => self.resolve_type_ref(ty_ref).unwrap_or(self.unit),
            None => self.unit,
        };
        let modifiers = self.validate_callable_modifiers(
            decl,
            self.extension_receivers.get(&id).copied(),
            false,
            matches!(self.functions[id].kind, FunctionKind::User(_)),
            &params,
            return_ty,
        );
        self.type_params_in_scope.clear();

        self.functions[id].return_ty = return_ty;
        self.functions[id].modifiers = modifiers;
        self.signatures.insert(
            id,
            FnSig {
                result_pending: crate::declaration_inference::infers_header_result(decl),
                parameter_pending: false,
                context_parameters: self.functions[id].context_parameters.clone(),
                is_suspend: decl.is_suspend,
                modifiers,
                attributes: self.functions[id].attributes,
                owner_type_param_count: 0,
                type_params,
                params,
                return_ty,
            },
        );
        if let FunctionKind::Extern(extern_id) = self.functions[id].kind {
            let signature = &self.signatures[&id];
            self.extern_functions[extern_id].params =
                signature.params.iter().map(|param| param.ty).collect();
            self.extern_functions[extern_id].result =
                scoop_identity::ExternResult::Direct(return_ty);
        }
    }
}
