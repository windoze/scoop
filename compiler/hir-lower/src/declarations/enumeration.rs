use super::*;

mod integral;

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
        let mut type_params = self.local_type_parameters();
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
            let parameter = self.fresh_source_type_param(param, type_params.len());
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
            methods: Vec::new(),
            properties: Vec::new(),
            derived_equality: None,
            span: decl.span,

            definition: hir::EnumDefinition {
                representation: hir::EnumRepresentation::Algebraic,
                no_gc,
                self_application,
                type_params,
                variants: Vec::new(),
                interfaces: Vec::new(),
                interface_implementations: Vec::new(),

                gc_free_pointee_requirements: Vec::new(),
            },
        });
        self.register_nominal_identity(Owner::Enum(id), identity);
        self.register_nominal_name(
            owner,
            &decl.name,
            NominalTarget::Enum(id),
            file_index,
            decl.visibility,
        );
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
        self.record_nominal_file(Owner::Enum(id), file_index);
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
