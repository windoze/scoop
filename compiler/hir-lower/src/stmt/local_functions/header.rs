use super::*;

impl Lowerer {
    pub(super) fn prepare_local_function(
        &mut self,
        decl: ast::FunctionHeaderRef<'_>,
    ) -> Option<hir::LocalFunctionId> {
        if let Some(operator) = decl.operator {
            self.error(
                operator.span,
                "`operator` requires a member or extension receiver".to_string(),
            );
        }
        if let Some(infix) = decl.infix {
            self.error(
                infix.span,
                "`infix` requires a member or extension receiver".to_string(),
            );
        }
        let outer_type_params = self.type_params_in_scope.clone();
        let owner_type_param_count = outer_type_params.len();
        let mut type_params = outer_type_params.clone();
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
            type_params.push(crate::lower_type_param_decl(param, parameter));
        }
        type_params = self.resolve_type_parameter_constraints(
            type_params,
            owner_type_param_count,
            &decl.type_params,
            decl.where_clause.as_ref(),
            true,
            "local function",
        );
        self.type_params_in_scope = type_params.clone();
        let mut sig_params = Vec::with_capacity(decl.params.len());
        for param in &decl.params {
            sig_params.push(self.resolve_fn_param(param)?);
        }
        let return_ty = match &decl.return_ty {
            Some(ty) => self.resolve_type_ref(ty)?,
            None => self.unit,
        };
        let context_parameters = self.resolve_context_parameters(decl.signature);
        let function_type = self.intern_function_type(
            decl.is_suspend,
            sig_params.iter().map(|param| param.ty).collect(),
            return_ty,
        );
        let attributes = self
            .check_function_annotations(decl, crate::FunctionTarget::Local)
            .attributes;
        let definition_path = self
            .definition_paths
            .next(scoop_identity::StructuralDefinitionSiteRole::LocalDeclaration);
        let local_number = self.local_function_by_function.len();
        let access = self.local_declaration_access();
        let function = self.functions.alloc(hir::Function {
            signature: hir::CallableSignature {
                context_parameters,
                release_callability: Default::default(),
                name: format!("$local.{local_number}.{}", decl.name.text),
                is_suspend: decl.is_suspend,
                modifiers: hir::CallableModifiers::default(),
                params: Vec::new(),
                return_ty,
                attributes,
                span: decl.span,
            },

            access,
            genericity: hir::FunctionGenericity::Plain,
            kind: hir::FunctionKind::User(hir::Body {
                locals: la_arena::Arena::new(),
                statements: Vec::new(),
            }),
            method: None,
        });
        self.source_function_declarations.insert(
            function,
            crate::SourceFunctionDeclaration {
                name: decl.name.text.clone(),
                name_span: Some(decl.name.span),
            },
        );
        self.record_function_file(function, self.current_file);
        self.signatures.insert(
            function,
            FnSig {
                result_pending: crate::declaration_inference::infers_header_result(decl),
                parameter_pending: false,
                context_parameters: self.functions[function].context_parameters.clone(),
                is_suspend: decl.is_suspend,
                modifiers: hir::CallableModifiers::default(),
                attributes,
                owner_type_param_count,
                type_params: type_params.clone(),
                params: sig_params.clone(),
                return_ty,
            },
        );
        if !type_params.is_empty() {
            self.register_generic(function, type_params.clone());
        }
        self.annotate_function(function, decl);
        let local = self.local_functions.alloc(hir::LocalFunction {
            definition: hir::LexicalFunctionDefinition::Source {
                function,
                root: self.current_definition_root(),
            },
            definition_path: definition_path.clone(),
            declaration_function_type: function_type,
            function_type,
            captures: Vec::new(),
            owner_type_param_count,
            origin: self.definition_origin(decl.span),
            span: decl.span,
        });
        self.local_function_by_function.insert(function, local);
        self.observe_local_function(function, decl);

        let duplicate = self
            .local_function_scopes
            .current(&decl.name.text)
            .into_iter()
            .map(|candidate| self.local_functions[candidate].source_function())
            .any(|candidate| self.same_parameter_signature(function, candidate));
        if duplicate {
            self.error(
                decl.name.span,
                format!(
                    "local function `{}` is already declared with the same signature",
                    decl.name.text
                ),
            );
        }
        // Declaration-before-use plus self visibility: insert the entity only
        // after its signature is complete and immediately before its body.
        if !duplicate {
            self.local_function_scopes
                .declare(decl.name.text.clone(), local);
        }

        Some(local)
    }
}
