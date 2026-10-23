use super::*;
use la_arena::Arena;

impl Lowerer {
    /// Resolve a member function's signature (pass 2.5). The implicit
    /// `this` is not part of the `FnSig` (calls are checked against the
    /// declared parameters only); it becomes `params[0]` of the
    /// `hir::Function` when its body or abstract parameter environment is built.
    pub(crate) fn resolve_method_signature(
        &mut self,
        id: FunctionId,
        decl: &ast::FunctionDecl,
        owner: Owner,
    ) {
        self.resolve_method_header(id, decl.header(), owner);
        self.register_inferred_function(id, decl);
    }

    pub(crate) fn resolve_method_header(
        &mut self,
        id: FunctionId,
        decl: ast::FunctionHeaderRef<'_>,
        owner: Owner,
    ) {
        if !self.editing_owner_available(owner) {
            self.declaration_surface
                .mark_signature_unavailable(id, decl);
            return;
        }
        let diagnostics_before = self.diagnostics.len();
        let short = decl.name.text.clone();
        if !decl.type_params.is_empty() {
            if matches!(owner, Owner::Interface(_)) {
                self.error(
                    decl.name.span,
                    format!("interface method `{short}` cannot declare method type parameters"),
                );
            }
            if matches!(owner, Owner::Class(_)) && decl.modifier != ast::MethodModifier::Final {
                self.error(
                    decl.name.span,
                    format!("generic member function `{short}` in a class must be final"),
                );
            }
            if decl.is_override {
                self.error(
                    decl.name.span,
                    format!("generic member function `{short}` cannot be an override"),
                );
            }
        }
        self.check_method_body_shape(id, decl, owner);

        let mut type_params = self.method_type_parameter_environment(id, owner);
        let owner_type_param_count = type_params.len();
        for param in &decl.type_params {
            if type_params
                .iter()
                .any(|existing| existing.name == param.name.text)
            {
                self.error(
                    param.span,
                    format!("duplicate type parameter `{}`", param.name.text),
                );
                continue;
            }
            let parameter = self.fresh_source_type_param(param, type_params.len());
            let param = crate::lower_type_param_decl(param, parameter);
            type_params.push(param.clone());
        }
        type_params = self.resolve_type_parameter_constraints(
            type_params,
            owner_type_param_count,
            &decl.type_params,
            decl.where_clause.as_ref(),
            true,
            "method",
        );
        let mut owner_parameters = type_params.clone();
        let method_parameters = owner_parameters.split_off(owner_type_param_count);
        self.register_method_parameters(id, owner_parameters, method_parameters);
        self.type_params_in_scope = type_params.clone();
        let params = self.resolve_callable_parameters(decl.signature);
        self.functions[id].context_parameters = self.resolve_context_parameters(decl.signature);
        let return_ty = match &decl.return_ty {
            Some(ty_ref) => self.resolve_type_ref(ty_ref).unwrap_or(self.unit),
            None => self.unit,
        };
        let receiver_ty = self.owner_ty(owner);
        let modifiers = self.validate_callable_modifiers(
            decl,
            Some(receiver_ty),
            true,
            matches!(
                self.functions[id].kind,
                hir::FunctionKind::User(_) | hir::FunctionKind::Abstract { .. }
            ),
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
                owner_type_param_count,
                type_params,
                params,
                return_ty,
            },
        );

        // Abstract declarations retain only their parameter environment;
        // executable methods are lowered in pass 3.
        let host_ty = self.owner_ty(owner);
        if !matches!(self.functions[id].kind, hir::FunctionKind::Intrinsic(_))
            && (decl.modifier == ast::MethodModifier::Abstract
                || decl.body_form == ast::FunctionBodyForm::Omitted)
        {
            let locals = self.build_abstract_parameter_locals(id, host_ty);
            self.functions[id].kind = hir::FunctionKind::Abstract { locals };
        }
        if self.diagnostics.len() != diagnostics_before {
            self.declaration_surface
                .mark_signature_unavailable(id, decl);
        }
    }

    /// Body-shape rules for member declarations: `abstract` only in
    /// abstract classes, interface methods may be abstract or default, and
    /// concrete class/value methods always have a body.
    fn check_method_body_shape(
        &mut self,
        id: FunctionId,
        decl: ast::FunctionHeaderRef<'_>,
        owner: Owner,
    ) {
        if matches!(self.functions[id].kind, hir::FunctionKind::Intrinsic(_)) {
            return;
        }
        let short = decl.name.text.clone();
        match owner {
            Owner::Interface(_) => {
                if self.functions[id].access.declared == hir::DeclaredVisibility::Private
                    && decl.body_form == ast::FunctionBodyForm::Omitted
                {
                    self.error(
                        decl.name.span,
                        format!("private interface method `{short}` must have a body"),
                    );
                }
            }
            Owner::Class(class_id) => {
                let abstract_class =
                    self.classes[class_id].modifier == hir::ClassModifier::Abstract;
                if decl.modifier == ast::MethodModifier::Abstract && !abstract_class {
                    self.error(
                        decl.name.span,
                        format!("abstract function `{short}` is only allowed in abstract classes"),
                    );
                }
                if decl.modifier == ast::MethodModifier::Open
                    && !decl.is_override
                    && self.classes[class_id].modifier == hir::ClassModifier::Final
                {
                    self.error(
                        decl.name.span,
                        format!(
                            "open function `{short}` is only allowed in open or abstract classes"
                        ),
                    );
                }
                match (
                    decl.body_form,
                    decl.modifier == ast::MethodModifier::Abstract,
                ) {
                    (ast::FunctionBodyForm::Omitted, false) => self.error(
                        decl.name.span,
                        format!("function `{short}` must have a body"),
                    ),
                    (ast::FunctionBodyForm::Block | ast::FunctionBodyForm::Expression, true) => {
                        self.error(
                            decl.name.span,
                            format!("abstract function `{short}` must not have a body"),
                        )
                    }
                    _ => {}
                }
            }
            Owner::Struct(_) | Owner::Enum(_) => {
                if decl.modifier == ast::MethodModifier::Abstract {
                    self.error(
                        decl.name.span,
                        format!("abstract function `{short}` is only allowed in abstract classes"),
                    );
                }
                if decl.modifier == ast::MethodModifier::Open && !decl.is_override {
                    self.error(
                        decl.name.span,
                        format!("open function `{short}` is only allowed in class declarations"),
                    );
                }
                // `override` on a value-type method is checked in pass
                // 2.75 (it is required exactly when the method
                // implements an interface method, DESIGN.md 5.2).
                if decl.body_form == ast::FunctionBodyForm::Omitted {
                    self.error(
                        decl.name.span,
                        format!("function `{short}` must have a body"),
                    );
                }
            }
            Owner::Object(_) => {
                if decl.modifier == ast::MethodModifier::Abstract {
                    self.error(
                        decl.name.span,
                        format!("abstract function `{short}` is only allowed in abstract classes"),
                    );
                }
                if decl.modifier == ast::MethodModifier::Open && !decl.is_override {
                    self.error(
                        decl.name.span,
                        format!("open function `{short}` is not allowed in an object declaration"),
                    );
                }
                if decl.body_form == ast::FunctionBodyForm::Omitted
                    && decl.modifier != ast::MethodModifier::Abstract
                {
                    self.error(
                        decl.name.span,
                        format!("function `{short}` must have a body"),
                    );
                }
            }
        }
    }

    /// Bind `this` and declared parameters without constructing a source body.
    /// The receiver remains first in `Function::params`.
    fn build_abstract_parameter_locals(
        &mut self,
        id: FunctionId,
        host_ty: TypeId,
    ) -> Arena<hir::Local> {
        let sig = self.signatures[&id].clone();
        let this = self.alloc_this_local(host_ty, self.functions[id].span);
        let mut params = vec![hir::Param {
            name: "this".to_string(),
            ty: host_ty,
            local: this,
        }];
        for (index, param) in sig.params.iter().enumerate() {
            let local = self.alloc_parameter_local(
                param.name.text.clone(),
                param.ty,
                index,
                param.name.span,
            );
            params.push(hir::Param {
                name: param.name.text.clone(),
                ty: param.ty,
                local,
            });
        }
        self.functions[id].params = params;
        std::mem::take(&mut self.locals).into_arena()
    }
}
