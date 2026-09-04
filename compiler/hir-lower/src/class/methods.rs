use super::*;

impl Lowerer {
    /// Resolve a member function's signature (pass 2.5). The implicit
    /// `this` is not part of the `FnSig` (calls are checked against the
    /// declared parameters only); it becomes `params[0]` of the
    /// `hir::Function` when the body (or the parameter-only body of a
    /// bodyless declaration) is built.
    pub(crate) fn resolve_method_signature(
        &mut self,
        id: FunctionId,
        decl: &ast::FunctionDecl,
        owner: Owner,
    ) {
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

        let mut type_params = self.owner_type_params(owner);
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
            let parameter = self.fresh_type_param(type_params.len());
            let param = crate::lower_type_param_decl(param, parameter);
            type_params.push(param.clone());
        }
        type_params = self.resolve_type_parameter_constraints(
            type_params,
            owner_type_param_count,
            &decl.type_params,
            decl.where_clause.as_ref(),
            "method",
        );
        let mut owner_parameters = type_params.clone();
        let method_parameters = owner_parameters.split_off(owner_type_param_count);
        self.register_method_parameters(id, owner_parameters, method_parameters);
        self.type_params_in_scope = type_params.clone();
        let mut params = Vec::with_capacity(decl.params.len());
        for param in &decl.params {
            if let Some(param) = self.resolve_fn_param(param) {
                params.push(param);
            }
        }
        let return_ty = match &decl.return_ty {
            Some(ty_ref) => self.resolve_type_ref(ty_ref).unwrap_or(self.unit),
            None => self.unit,
        };
        self.type_params_in_scope.clear();

        let receiver_ty = self.owner_ty(owner);
        let modifiers =
            self.validate_callable_modifiers(decl, Some(receiver_ty), true, &params, return_ty);

        self.functions[id].return_ty = return_ty;
        self.functions[id].modifiers = modifiers;
        self.signatures.insert(
            id,
            FnSig {
                is_suspend: decl.is_suspend,
                modifiers,
                attributes: self.functions[id].attributes,
                owner_type_param_count,
                type_params,
                params,
                return_ty,
            },
        );

        // Bodyless declarations get their parameter-only body here;
        // concrete methods are lowered in pass 3.
        let host_ty = self.owner_ty(owner);
        if !matches!(self.functions[id].kind, hir::FunctionKind::Intrinsic(_))
            && (decl.modifier == ast::MethodModifier::Abstract
                || matches!(decl.body, ast::FunctionBody::None))
        {
            let (body, _) = self.build_params_only_body(id, host_ty);
            self.functions[id].kind = hir::FunctionKind::User(body);
        }
    }

    /// Body-shape rules for member declarations: `abstract` only in
    /// abstract classes, interface methods may be abstract or default, and
    /// concrete class/value methods always have a body.
    fn check_method_body_shape(&mut self, id: FunctionId, decl: &ast::FunctionDecl, owner: Owner) {
        if matches!(self.functions[id].kind, hir::FunctionKind::Intrinsic(_)) {
            return;
        }
        let short = decl.name.text.clone();
        match owner {
            Owner::Interface(_) => {
                if self.functions[id].access.declared == hir::DeclaredVisibility::Private
                    && matches!(decl.body, ast::FunctionBody::None)
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
                match (&decl.body, decl.modifier == ast::MethodModifier::Abstract) {
                    (ast::FunctionBody::None, false) => self.error(
                        decl.name.span,
                        format!("function `{short}` must have a body"),
                    ),
                    (ast::FunctionBody::Block(_) | ast::FunctionBody::Expr(_), true) => self.error(
                        decl.name.span,
                        format!("abstract function `{short}` must not have a body"),
                    ),
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
                if matches!(decl.body, ast::FunctionBody::None) {
                    self.error(
                        decl.name.span,
                        format!("function `{short}` must have a body"),
                    );
                }
            }
        }
    }

    /// The body of a bodyless declaration (interface / abstract
    /// method): locals for `this` and the declared parameters, no
    /// statements. Fills `Function::params` (receiver first) and
    /// returns a second copy of the declared-parameter list for the
    /// interface's `MethodSig` (`hir::Param` is not `Clone`; both
    /// copies refer to the same locals of this body).
    fn build_params_only_body(
        &mut self,
        id: FunctionId,
        host_ty: TypeId,
    ) -> (hir::Body, Vec<hir::Param>) {
        let sig = self.signatures[&id].clone();
        let this = self.alloc_local("this".to_string(), host_ty, false);
        let mut params = vec![hir::Param {
            name: "this".to_string(),
            ty: host_ty,
            local: this,
        }];
        let mut declared = Vec::with_capacity(sig.params.len());
        for param in &sig.params {
            let local = self.alloc_local(param.name.text.clone(), param.ty, false);
            params.push(hir::Param {
                name: param.name.text.clone(),
                ty: param.ty,
                local,
            });
            declared.push(hir::Param {
                name: param.name.text.clone(),
                ty: param.ty,
                local,
            });
        }
        self.functions[id].params = params;
        let body = hir::Body {
            locals: std::mem::take(&mut self.locals),
            statements: Vec::new(),
        };
        (body, declared)
    }
}
