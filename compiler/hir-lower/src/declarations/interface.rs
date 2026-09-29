use super::*;

impl Lowerer {
    pub(crate) fn declare_interface<'a>(
        &mut self,
        decl: &'a ast::InterfaceDecl,
        pending: &mut Vec<(InterfaceId, &'a ast::InterfaceDecl, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
        owner: Option<Owner>,
    ) -> Option<InterfaceId> {
        self.reject_type_annotations("an interface", &decl.annotations);
        if let Some(kind) = self.type_namespace_conflict(
            owner,
            &decl.name.text,
            file_index,
            crate::namespace::is_file_private(decl.visibility),
        ) {
            let what = if kind == "an interface" {
                format!("duplicate interface `{}`", decl.name.text)
            } else {
                format!(
                    "duplicate type `{}` (already declared as {kind})",
                    decl.name.text
                )
            };
            self.error(decl.name.span, what);
            return None;
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
        let self_application = hir::InterfaceApplicationId::from_raw(
            (self.interface_applications.len() as u32).into(),
        );
        let access = match owner {
            Some(owner) => self.nested_nominal_access(
                decl.visibility,
                decl.name.span,
                "interface",
                owner,
                file_index,
            ),
            None => self.nominal_access(decl.visibility, decl.name.span, "interface", file_index),
        };
        let identity = self.prepare_nominal_identity(NominalIdentityInput {
            name: &decl.name.text,
            parent: owner,
            access: access.declared,
            type_parameter_count: type_params.len(),
            kind: SourceNominalKind::Interface,
            file: file_index,
            span: decl.span,
        })?;
        let id = self.interfaces.alloc(InterfaceDecl {
            name: decl.name.text.clone(),
            owner: owner.map(Owner::as_nominal_owner),
            access,
            self_application,
            type_params: type_params.clone(),
            gc_free_pointee_requirements: Vec::new(),
            parents: Vec::new(),
            // Filled in pass 2.5 together with the method signatures.
            methods: Vec::new(),
            private_methods: Vec::new(),
            properties: Vec::new(),
            span: decl.span,
        });
        self.register_nominal_identity(Owner::Interface(id), identity);
        let parameter_ids = type_params
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let type_args = parameter_ids
            .into_iter()
            .map(|parameter| self.intern_type(Type::Param(parameter)))
            .collect();
        let ty = self.intern_interface_application(id, type_args);
        assert_eq!(self.types[ty], Type::Interface(self_application));
        match owner {
            Some(owner) => {
                self.nested_nominals_by_owner.insert(
                    (owner, decl.name.text.clone()),
                    NominalTarget::Interface(id),
                );
            }
            None => {
                self.top_level_namespaces.register_type(
                    file_index,
                    decl.name.text.clone(),
                    crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Interface(id)),
                    self.interfaces[id].access.declared == hir::DeclaredVisibility::Private,
                );
            }
        }
        self.interface_files.insert(id, file_index);
        self.interface_methods.insert(id, Vec::new());
        for method in &decl.methods {
            self.declare_method(method, Owner::Interface(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
        Some(id)
    }
}
