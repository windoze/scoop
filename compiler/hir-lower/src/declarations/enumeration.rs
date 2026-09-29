use super::*;

impl Lowerer {
    pub(crate) fn declare_enum<'a>(
        &mut self,
        decl: &'a ast::EnumDecl,
        is_core: bool,
        pending: &mut Vec<(EnumId, &'a ast::EnumDecl, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
        owner: Option<Owner>,
    ) -> Option<EnumId> {
        let no_gc = self.check_enum_annotations(decl);
        if let Some(kind) = self.type_namespace_conflict(
            owner,
            &decl.name.text,
            file_index,
            crate::namespace::is_file_private(decl.visibility),
        ) {
            let what = if kind == "an enum" {
                format!("duplicate enum `{}`", decl.name.text)
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
        let self_application =
            hir::EnumApplicationId::from_raw((self.enum_applications.len() as u32).into());
        let access = match owner {
            Some(owner) => self.nested_nominal_access(
                decl.visibility,
                decl.name.span,
                "enum",
                owner,
                file_index,
            ),
            None => self.nominal_access(decl.visibility, decl.name.span, "enum", file_index),
        };
        let identity = self.prepare_nominal_identity(NominalIdentityInput {
            name: &decl.name.text,
            parent: owner,
            access: access.declared,
            type_parameter_count: type_params.len(),
            kind: SourceNominalKind::Enum,
            file: file_index,
            span: decl.span,
        })?;
        let id = self.enums.alloc(EnumDecl {
            name: decl.name.text.clone(),
            owner: owner.map(Owner::as_nominal_owner),
            access,
            self_application,
            type_params,
            gc_free_pointee_requirements: Vec::new(),
            no_gc,
            // Filled in pass 2; a resolution failure is diagnosed, so
            // empty variants never reach the output.
            variants: Vec::new(),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            properties: Vec::new(),
            derived_equality: None,
            span: decl.span,
        });
        self.register_nominal_identity(Owner::Enum(id), identity);
        match owner {
            Some(owner) => {
                self.nested_nominals_by_owner
                    .insert((owner, decl.name.text.clone()), NominalTarget::Enum(id));
            }
            None => {
                self.top_level_namespaces.register_type(
                    file_index,
                    decl.name.text.clone(),
                    crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Enum(id)),
                    self.enums[id].access.declared == hir::DeclaredVisibility::Private,
                );
            }
        }
        let parameter_ids = self.enums[id]
            .type_params
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let type_args = parameter_ids
            .into_iter()
            .map(|parameter| self.intern_type(Type::Param(parameter)))
            .collect();
        let ty = self.enum_application(id, type_args);
        assert_eq!(self.types[ty], Type::Enum(self_application));
        self.enum_files.insert(id, file_index);
        if is_core && owner.is_none() && decl.name.text == "Option" {
            self.option_candidates
                .push((id, file_index, decl.span, decl.type_params.len()));
        }
        for method in &decl.methods {
            self.declare_method(method, Owner::Enum(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
        Some(id)
    }
}
