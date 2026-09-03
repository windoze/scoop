use super::*;

mod patching;

use patching::patch_local_function_calls;

impl Lowerer {
    pub(super) fn lower_local_function_decl(
        &mut self,
        decl: &ast::FunctionDecl,
    ) -> Option<hir::LocalFunctionId> {
        if let Some(operator) = decl.operator {
            self.error(
                operator.span,
                "`operator` is only allowed on member functions".to_string(),
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
            let parameter = self.fresh_type_param(type_params.len());
            type_params.push(crate::lower_type_param_decl(param, parameter));
        }
        type_params = self.resolve_type_parameter_constraints(
            type_params,
            owner_type_param_count,
            &decl.type_params,
            decl.where_clause.as_ref(),
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
        let function_ty = self.intern_function_type(
            decl.is_suspend,
            sig_params.iter().map(|param| param.ty).collect(),
            return_ty,
        );
        let Type::Function(function_type) = self.types[function_ty] else {
            unreachable!("interning a function type returns a function type")
        };
        let attributes = self
            .check_function_annotations(decl, crate::FunctionTarget::Local)
            .attributes;
        let local_number = self.local_functions.len();
        let function = self.functions.alloc(hir::Function {
            name: format!("$local.{local_number}.{}", decl.name.text),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: decl.is_suspend,
            params: Vec::new(),
            return_ty,
            attributes,
            kind: hir::FunctionKind::User(hir::Body {
                locals: la_arena::Arena::new(),
                statements: Vec::new(),
            }),
            method: None,
            span: decl.span,
        });
        self.signatures.insert(
            function,
            FnSig {
                is_suspend: decl.is_suspend,
                operator: None,
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
        let local = self.local_functions.alloc(hir::LocalFunction {
            function,
            function_type,
            captures: Vec::new(),
            owner_type_param_count,
            span: decl.span,
        });
        self.local_function_by_function.insert(function, local);

        let duplicate = self
            .local_function_scopes
            .current(&decl.name.text)
            .into_iter()
            .map(|candidate| self.local_functions[candidate].function)
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

        let capture_environment = self.capture_environment();
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_scopes = std::mem::replace(&mut self.scopes, Scopes::new());
        let outer_return_ty = self.current_return_ty;
        let outer_return_inference = self.return_inference.take();
        let outer_fn_name = std::mem::take(&mut self.current_fn_name);
        let outer_this = self.current_this.take();
        let outer_smart_casts = std::mem::take(&mut self.smart_casts);
        self.capture_contexts.push(CaptureContext {
            available: capture_environment,
            captures: Vec::new(),
            by_binding: std::collections::HashMap::new(),
        });
        self.current_return_ty = return_ty;
        self.current_fn_name = self.functions[function].name.clone();
        self.push_suspension_context(if decl.is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::Function)
        });
        self.push_safety_context(attributes.safety);
        self.push_scope();

        let lowered = {
            let mut params = Vec::with_capacity(sig_params.len());
            for param in &sig_params {
                if self.scopes.is_declared_here(&param.name.text) {
                    self.error(
                        param.name.span,
                        format!("duplicate parameter `{}`", param.name.text),
                    );
                    continue;
                }
                let local = self.alloc_local(param.name.text.clone(), param.ty, false);
                self.scopes.declare(param.name.text.clone(), local);
                params.push(hir::Param {
                    name: param.name.text.clone(),
                    ty: param.ty,
                    local,
                });
            }
            let returns_unit = self.types_equal(return_ty, self.unit);
            let mut statements = match &decl.body {
                ast::FunctionBody::Block(block) => {
                    let diagnostics_before = self.diagnostics.len();
                    let statements = self.lower_block(block);
                    if !returns_unit
                        && self.diagnostics.len() == diagnostics_before
                        && statements_can_fall_through(&statements)
                    {
                        self.error(
                            block.span,
                            format!(
                                "non-Unit local function `{}` may complete without returning a value",
                                decl.name.text
                            ),
                        );
                    }
                    statements
                }
                ast::FunctionBody::Expr(expr) => {
                    let mut statements = Vec::new();
                    let mut sink = Vec::new();
                    if let Some(value) = self.lower_expr(expr, &mut sink, Some(return_ty)) {
                        if !self.is_subtype(value.ty, return_ty) {
                            let expected = self.type_name(return_ty);
                            let found = self.type_name(value.ty);
                            self.error(
                                expr.span(),
                                format!(
                                    "body of local function `{}` must be of type {expected}, found {found}",
                                    decl.name.text
                                ),
                            );
                        } else {
                            statements.extend(sink);
                            let value = self.adapt_to(value, return_ty);
                            self.push_return(Some(value), expr.span(), &mut statements);
                        }
                    }
                    statements
                }
                ast::FunctionBody::None => {
                    self.error(
                        decl.name.span,
                        format!("local function `{}` must have a body", decl.name.text),
                    );
                    Vec::new()
                }
            };
            let captures = self.finish_current_captures();
            patch_local_function_calls(&mut statements, local, &captures);
            let mut abi_params = Vec::with_capacity(captures.len() + params.len());
            for capture in &captures {
                let capture_local =
                    self.alloc_local(format!("$capture.{}", capture.name), capture.ty, false);
                abi_params.push(hir::Param {
                    name: format!("$capture.{}", capture.name),
                    ty: capture.ty,
                    local: capture_local,
                });
            }
            abi_params.extend(params);
            let body_locals = std::mem::take(&mut self.locals);
            Some((statements, captures, abi_params, body_locals))
        };

        self.pop_scope();
        self.pop_safety_context();
        self.pop_suspension_context();
        self.capture_contexts.pop();
        self.locals = outer_locals;
        self.scopes = outer_scopes;
        self.current_return_ty = outer_return_ty;
        self.return_inference = outer_return_inference;
        self.current_fn_name = outer_fn_name;
        self.current_this = outer_this;
        self.smart_casts = outer_smart_casts;
        self.type_params_in_scope = outer_type_params;

        let (statements, captures, params, body_locals) = lowered?;
        self.functions[function].params = params;
        self.functions[function].kind = hir::FunctionKind::User(hir::Body {
            locals: body_locals,
            statements,
        });
        self.local_functions[local].captures = captures;
        Some(local)
    }
}
