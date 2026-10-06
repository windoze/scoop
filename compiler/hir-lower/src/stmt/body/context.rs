use super::*;

impl Lowerer {
    /// Establish the ordinary function scope for source and generated bodies.
    pub(super) fn with_function_body(
        &mut self,
        id: FunctionId,
        name: &str,
        lower: impl FnOnce(&mut Self) -> Vec<hir::Statement>,
    ) -> hir::Body {
        let outer_loop_targets = std::mem::take(&mut self.loop_targets);
        let outer_source_context = self.current_source_context;
        let outer_definition_paths = std::mem::take(&mut self.definition_paths);
        let outer_definition_root = self
            .definition_root
            .replace(hir::LexicalDefinitionRoot::Function(id));
        let sig = self.signatures[&id].clone();
        // Member functions (M6): `this` is parameter 0, an immutable
        // local of the host type; bare property / method names in the
        // body resolve against it.
        let owner = self.function_owner.get(&id).copied();
        // Method signatures already carry the combined owner-prefix plus
        // method-suffix namespace established in pass 2.5.
        self.type_params_in_scope = sig.type_params.clone();
        self.current_return_ty = sig.return_ty;
        self.current_fn_name = name.to_owned();
        self.push_suspension_context(if self.functions[id].is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::Function)
        });
        self.push_safety_context(self.functions[id].attributes.safety);

        self.current_owner = owner;
        self.current_this = None;
        self.set_source_context(hir::SourceContextSubject::Function(id));

        // Parameters are immutable locals in the function's outermost
        // scope; the body block nests inside it, so body locals may
        // shadow parameters.
        self.push_scope();
        let extension_receiver = self.extension_receivers.get(&id).copied();
        let mut params = Vec::with_capacity(
            sig.params.len() + usize::from(owner.is_some() || extension_receiver.is_some()),
        );
        if let Some(host_ty) = self.functions[id]
            .method
            .map(|method| method.owner)
            .or_else(|| owner.map(|owner| self.owner_ty(owner)))
            .or(extension_receiver)
        {
            let local = self.alloc_this_local(host_ty, self.functions[id].span);
            self.scopes.declare("this".to_string(), local);
            self.current_this = Some((local, host_ty));
            params.push(hir::Param {
                name: "this".to_string(),
                ty: host_ty,
                local,
            });
        }
        for (index, param) in sig.params.iter().enumerate() {
            if self.scopes.is_declared_here(&param.name.text) {
                self.error(
                    param.name.span,
                    format!("duplicate parameter `{}`", param.name.text),
                );
                continue;
            }
            let local = self.alloc_parameter_local(
                param.name.text.clone(),
                param.ty,
                index,
                param.name.span,
            );
            self.scopes.declare(param.name.text.clone(), local);
            params.push(hir::Param {
                name: param.name.text.clone(),
                ty: param.ty,
                local,
            });
        }
        self.functions[id].params = params;
        let mut entry = self.lower_context_entry(id);

        let statements = lower(self);
        self.pop_scope();
        self.type_params_in_scope.clear();
        self.current_this = None;
        self.current_owner = None;
        self.current_source_context = outer_source_context;
        self.definition_paths = outer_definition_paths;
        self.definition_root = outer_definition_root;
        self.pop_safety_context();
        self.pop_suspension_context();
        debug_assert!(self.loop_targets.is_empty());
        self.loop_targets = outer_loop_targets;

        hir::Body {
            locals: std::mem::take(&mut self.locals),
            statements: {
                entry.extend(statements);
                entry
            },
        }
    }
}
